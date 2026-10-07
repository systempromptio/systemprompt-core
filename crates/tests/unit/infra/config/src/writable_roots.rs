//! The writable-path contract behind a read-only root filesystem: which
//! directories each node role declares, and the boot probe that names every
//! one it cannot write.

use std::path::{Path, PathBuf};

use systemprompt_config::{AppPaths, WritableRoot, ensure_state_dirs_writable};
use systemprompt_manifest::paths::PathResolution;
use systemprompt_manifest::profile::{NodeRole, PathsConfig};

fn app_paths(root: &Path) -> AppPaths {
    let cfg = PathsConfig {
        system: root.join("system").display().to_string(),
        services: root.join("services").display().to_string(),
        bin: root.join("bin").display().to_string(),
        web_path: Some(root.join("web").display().to_string()),
        storage: Some(root.join("storage").display().to_string()),
        geoip_database: None,
    };
    AppPaths::from_profile(&cfg, PathResolution::Lexical, None).expect("lexical resolve")
}

fn names(roots: &[WritableRoot]) -> Vec<&'static str> {
    roots.iter().map(|r| r.name).collect()
}

#[test]
fn worker_roles_declare_every_runtime_writer() {
    let paths = app_paths(Path::new("/app"));
    let cache = PathBuf::from("/app/system/services-cache");
    for role in [NodeRole::All, NodeRole::Admin] {
        assert_eq!(
            names(&paths.writable_roots(role, &cache)),
            vec![
                "system.logs",
                "storage.files",
                "storage.exports",
                "storage.data",
                "storage.scratch",
                "web.dist",
                "services.cache_dir",
            ]
        );
    }
}

#[test]
fn gateway_role_needs_no_web_dist_or_services_cache() {
    let paths = app_paths(Path::new("/app"));
    let roots = paths.writable_roots(NodeRole::Gateway, Path::new("/app/system/services-cache"));
    assert_eq!(
        names(&roots),
        vec![
            "system.logs",
            "storage.files",
            "storage.exports",
            "storage.data",
            "storage.scratch",
        ]
    );
    let scratch = roots
        .iter()
        .find(|r| r.name == "storage.scratch")
        .expect("scratch root");
    assert_eq!(scratch.path, PathBuf::from("/app/storage/data/scratch"));
}

#[test]
fn the_probe_creates_every_root_it_can_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = app_paths(dir.path());
    let roots = paths.writable_roots(NodeRole::All, &dir.path().join("cache"));

    ensure_state_dirs_writable(&roots).expect("every root is writable");

    for root in &roots {
        assert!(root.path.is_dir(), "{} was not created", root.name);
        let leftovers = std::fs::read_dir(&root.path)
            .expect("readable")
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("write-probe"))
            .count();
        assert_eq!(leftovers, 0, "{} kept its probe file", root.name);
    }
}

// Why: a regular file where a directory must be fails `create_dir_all` even
// for uid 0, so the refusal is exercised however the suite is run.
#[test]
fn the_probe_names_every_root_it_cannot_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = app_paths(dir.path());
    let roots = paths.writable_roots(NodeRole::Gateway, &dir.path().join("cache"));
    let storage = dir.path().join("storage");
    std::fs::create_dir_all(&storage).expect("storage");
    std::fs::write(storage.join("data"), b"not a directory").expect("blocker");
    std::fs::write(storage.join("exports"), b"not a directory").expect("blocker");

    let error = ensure_state_dirs_writable(&roots).expect_err("blocked roots refuse");

    let names: Vec<_> = error.failures.iter().filter_map(|f| f.name).collect();
    assert_eq!(
        names,
        vec!["storage.exports", "storage.data", "storage.scratch"]
    );
    let message = error.to_string();
    assert!(
        message.contains(&format!("storage.data {}", storage.join("data").display())),
        "{message}"
    );
    assert!(message.contains("storage.exports"), "{message}");
    assert!(!message.contains("storage.files"), "{message}");
}
