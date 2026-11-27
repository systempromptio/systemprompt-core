use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use systemprompt_engine::lsp::{LspConfig, LspManager, ServerId, ServerSpec, builtin_servers};

fn spec(id: &str, extensions: &[&str], markers: &[&str]) -> ServerSpec {
    ServerSpec {
        id: ServerId::new(id),
        command: id.to_owned(),
        args: Vec::new(),
        env: BTreeMap::new(),
        extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
        root_markers: markers.iter().map(|m| (*m).to_owned()).collect(),
        initialization_options: None,
    }
}

fn fake_program(dir: &Path, name: &str) {
    if cfg!(windows) {
        std::fs::write(dir.join(format!("{name}.exe")), "").unwrap();
    } else {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        make_executable(&path);
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn search_path(dir: &Path) -> OsString {
    std::env::join_paths([dir]).unwrap()
}

fn ids(manager: &LspManager) -> Vec<String> {
    manager
        .available_servers()
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect()
}

#[test]
fn matches_extensions_case_insensitively_with_or_without_dot() {
    let ts = spec("ts", &[".ts", "tsx"], &[]);
    assert!(ts.handles(Path::new("/w/a.ts")));
    assert!(ts.handles(Path::new("/w/A.TSX")));
    assert!(!ts.handles(Path::new("/w/a.rs")));
    assert!(!ts.handles(Path::new("/w/ts")));
    let make = spec("make", &["Makefile"], &[]);
    assert!(make.handles(Path::new("/w/sub/Makefile")));
}

#[test]
fn the_builtin_table_names_each_server_once() {
    let builtins = builtin_servers();
    assert!(
        builtins
            .iter()
            .any(|server| server.id.as_str() == "rust-analyzer")
    );
    let mut seen: Vec<&str> = builtins.iter().map(|server| server.id.as_str()).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), builtins.len());
}

#[test]
fn builtins_only_when_on_path_and_groups_pick_one() {
    let bin = tempfile::tempdir().unwrap();
    for name in [
        "rust-analyzer",
        "pyright-langserver",
        "basedpyright-langserver",
        "gopls",
    ] {
        fake_program(bin.path(), name);
    }
    let worktree = tempfile::tempdir().unwrap();
    let base = LspConfig {
        search_path: Some(search_path(bin.path())),
        ..LspConfig::default()
    };

    let all = LspManager::new(worktree.path(), base.clone());
    assert_eq!(ids(&all), vec!["rust-analyzer", "gopls", "basedpyright"]);

    let config = LspConfig {
        disabled: vec![ServerId::new("gopls")],
        ..base.clone()
    };
    assert_eq!(
        ids(&LspManager::new(worktree.path(), config)),
        vec!["rust-analyzer", "basedpyright"]
    );

    let config = LspConfig {
        builtins: false,
        ..base
    };
    assert_eq!(
        ids(&LspManager::new(worktree.path(), config)),
        Vec::<String>::new()
    );

    let unsearched = LspManager::new(worktree.path(), LspConfig::default());
    assert_eq!(ids(&unsearched), Vec::<String>::new());
}

#[test]
fn configured_servers_replace_builtins_and_take_their_group() {
    let bin = tempfile::tempdir().unwrap();
    fake_program(bin.path(), "rust-analyzer");
    fake_program(bin.path(), "basedpyright-langserver");
    let worktree = tempfile::tempdir().unwrap();
    let config = LspConfig {
        servers: vec![
            spec("rust-analyzer", &["rs"], &[]),
            spec("pyright", &["py"], &[]),
            spec("missing-ls", &["x"], &[]),
        ],
        search_path: Some(search_path(bin.path())),
        ..LspConfig::default()
    };
    let manager = LspManager::new(worktree.path(), config);
    assert_eq!(
        ids(&manager),
        vec!["rust-analyzer", "pyright", "missing-ls"]
    );
}

#[test]
fn selects_by_extension_and_nearest_root_marker() {
    let dir = tempfile::tempdir().unwrap();
    let worktree = dir.path();
    std::fs::create_dir_all(worktree.join("crates/a/src")).unwrap();
    std::fs::create_dir_all(worktree.join("web/app")).unwrap();
    std::fs::create_dir_all(worktree.join("dotnet/Proj")).unwrap();
    std::fs::write(worktree.join("Cargo.lock"), "").unwrap();
    std::fs::write(worktree.join("crates/a/Cargo.toml"), "").unwrap();
    std::fs::write(worktree.join("web/package.json"), "").unwrap();
    std::fs::write(worktree.join("dotnet/Proj/Proj.csproj"), "").unwrap();

    let config = LspConfig {
        builtins: false,
        servers: vec![
            spec("rust", &["rs"], &["Cargo.lock", "Cargo.toml"]),
            spec("cargo-member", &["rs"], &["Cargo.toml"]),
            spec("ts", &["ts"], &["package.json"]),
            spec("cs", &["cs"], &["*.sln", "*.csproj"]),
            spec("other", &["md"], &["nothing-here"]),
        ],
        ..LspConfig::default()
    };
    let manager = LspManager::new(worktree, config);
    let root: PathBuf = manager.worktree().to_path_buf();

    let picked: Vec<(String, PathBuf)> = manager
        .servers_for(Path::new("crates/a/src/lib.rs"))
        .into_iter()
        .map(|(id, server_root)| (id.as_str().to_owned(), server_root))
        .collect();
    assert_eq!(
        picked,
        vec![
            ("rust".to_owned(), root.clone()),
            ("cargo-member".to_owned(), root.join("crates/a"))
        ]
    );

    let picked = manager.servers_for(Path::new("web/app/main.ts"));
    assert_eq!(picked.len(), 1);
    assert_eq!(picked[0].1, root.join("web"));

    assert_eq!(
        manager.servers_for(Path::new("dotnet/Proj/Program.cs"))[0].1,
        root.join("dotnet/Proj")
    );
    assert_eq!(manager.servers_for(Path::new("web/README.md"))[0].1, root);

    assert_eq!(
        manager.servers_for(Path::new("notes.txt")),
        Vec::<(ServerId, PathBuf)>::new()
    );
    assert!(!manager.handles(Path::new("notes.txt")));
    assert!(manager.handles(Path::new("web/app/main.ts")));
    let elsewhere = if cfg!(windows) {
        r"Z:\elsewhere\lib.rs"
    } else {
        "/elsewhere/lib.rs"
    };
    assert_eq!(
        manager.servers_for(Path::new(elsewhere)),
        Vec::<(ServerId, PathBuf)>::new()
    );
}
