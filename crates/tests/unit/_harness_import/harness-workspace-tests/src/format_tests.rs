use std::ffi::OsString;
use std::fs;
use std::path::Path;

use harness_workspace::{Formatter, FormatterConfig, FormatterSpec};

fn names(specs: &[&FormatterSpec]) -> Vec<String> {
    specs.iter().map(|spec| spec.name.clone()).collect()
}

fn custom(name: &str, command: &str, extensions: &[&str]) -> FormatterSpec {
    FormatterSpec {
        name: name.to_owned(),
        command: command.to_owned(),
        args: vec!["$FILE".to_owned()],
        extensions: extensions.iter().map(|e| (*e).to_owned()).collect(),
        root_markers: Vec::new(),
        node_modules: false,
    }
}

#[cfg(unix)]
fn executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn builtins_are_matched_by_extension_in_priority_order() {
    let formatter = Formatter::new("/w", FormatterConfig::default());
    assert_eq!(
        names(&formatter.candidates(Path::new("src/main.rs"))),
        vec!["rustfmt"]
    );
    assert_eq!(
        names(&formatter.candidates(Path::new("a.py"))),
        vec!["ruff", "black"]
    );
    assert_eq!(
        names(&formatter.candidates(Path::new("App.TSX"))),
        vec!["biome", "prettier"]
    );
    assert_eq!(
        formatter.candidates(Path::new("README")),
        Vec::<&FormatterSpec>::new()
    );
}

#[test]
fn custom_specs_come_first_and_replace_builtins_of_the_same_name() {
    let config = FormatterConfig {
        custom: vec![
            custom("rustfmt", "my-rustfmt", &["rs"]),
            custom("mine", "x", &[".py"]),
        ],
        disabled: vec!["black".to_owned()],
        ..FormatterConfig::default()
    };
    let formatter = Formatter::new("/w", config);
    let rust = formatter.candidates(Path::new("lib.rs"));
    assert_eq!(names(&rust), vec!["rustfmt"]);
    assert_eq!(rust[0].command, "my-rustfmt");
    assert_eq!(
        names(&formatter.candidates(Path::new("a.py"))),
        vec!["mine", "ruff"]
    );
}

#[test]
fn builtins_can_be_turned_off() {
    let config = FormatterConfig {
        builtins: false,
        ..FormatterConfig::default()
    };
    assert_eq!(Formatter::new("/w", config).specs(), []);
}

#[test]
fn compound_extensions_match_whole_suffix() {
    let spec = custom("erb", "x", &[".html.erb"]);
    assert!(spec.handles("view.html.erb"));
    assert!(!spec.handles("view.erb"));
    assert!(!spec.handles(".html.erb"));
}

fn resolved_args(args: &[&str]) -> Vec<OsString> {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let program = root.join("tools").join("fmt-tool");
    fs::create_dir_all(program.parent().unwrap()).unwrap();
    fs::write(&program, "").unwrap();
    let mut spec = custom("args", program.to_str().unwrap(), &[".rs"]);
    spec.args = args.iter().map(|arg| (*arg).to_owned()).collect();
    let config = FormatterConfig {
        builtins: false,
        custom: vec![spec],
        ..FormatterConfig::default()
    };
    let file = root.join("a.rs");
    let resolved = Formatter::new(root, config).resolve(&file).unwrap();
    let file_text = file.to_string_lossy().into_owned();
    resolved
        .args
        .into_iter()
        .map(|arg| OsString::from(arg.to_string_lossy().replace(&file_text, "<FILE>")))
        .collect()
}

#[test]
fn file_placeholder_is_substituted_or_appended() {
    assert_eq!(
        resolved_args(&["-w", "$FILE"]),
        vec![OsString::from("-w"), OsString::from("<FILE>")]
    );
    assert_eq!(
        resolved_args(&["--check"]),
        vec![OsString::from("--check"), OsString::from("<FILE>")]
    );
    assert_eq!(
        resolved_args(&["--file=$FILE"]),
        vec![OsString::from("--file=<FILE>")]
    );
}

#[test]
fn missing_programs_resolve_to_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let empty_bin = dir.path().join("bin");
    fs::create_dir_all(&empty_bin).unwrap();
    let config = FormatterConfig {
        search_path: Some(empty_bin.into_os_string()),
        ..FormatterConfig::default()
    };
    let formatter = Formatter::new(dir.path(), config);
    assert!(formatter.resolve(&dir.path().join("main.go")).is_none());
}

#[test]
fn node_modules_shim_is_found_above_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let shim = if cfg!(windows) {
        "prettier.cmd"
    } else {
        "prettier"
    };
    let bin = root.join("node_modules").join(".bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join(shim), "").unwrap();
    fs::write(root.join(".prettierrc"), "{}").unwrap();
    fs::create_dir_all(root.join("web")).unwrap();

    let empty_bin = root.join("empty");
    fs::create_dir_all(&empty_bin).unwrap();
    let config = FormatterConfig {
        search_path: Some(empty_bin.into_os_string()),
        ..FormatterConfig::default()
    };
    let formatter = Formatter::new(root, config);
    let resolved = formatter.resolve(&root.join("web").join("app.ts")).unwrap();
    assert_eq!(resolved.name, "prettier");
    assert_eq!(resolved.program, bin.join(shim));
    assert_eq!(resolved.cwd, root);
}

#[test]
fn root_markers_gate_a_formatter() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let program = root.join("tools").join("fmt-tool");
    fs::create_dir_all(program.parent().unwrap()).unwrap();
    fs::write(&program, "").unwrap();
    let mut spec = custom("gated", program.to_str().unwrap(), &[".c"]);
    spec.root_markers = vec!["*.sln".to_owned()];
    let config = FormatterConfig {
        builtins: false,
        custom: vec![spec],
        ..FormatterConfig::default()
    };
    let formatter = Formatter::new(root, config);
    let file = root.join("src").join("a.c");
    assert!(formatter.resolve(&file).is_none());
    fs::write(root.join("Project.SLN"), "").unwrap();
    let resolved = formatter.resolve(&file).unwrap();
    assert_eq!(resolved.cwd, root);
    assert_eq!(resolved.program, program);
}

#[cfg(unix)]
#[tokio::test]
async fn format_reports_whether_the_file_changed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = root.join("bin").join("fake-fmt");
    executable(&script, "#!/bin/sh\nprintf 'formatted\\n' > \"$1\"\n");
    let config = FormatterConfig {
        builtins: false,
        custom: vec![custom("fake", script.to_str().unwrap(), &[".txt"])],
        ..FormatterConfig::default()
    };
    let formatter = Formatter::new(root, config);
    let file = root.join("note.txt");
    fs::write(&file, "messy").unwrap();
    assert!(formatter.format(&file).await.unwrap());
    assert_eq!(fs::read_to_string(&file).unwrap(), "formatted\n");
    assert!(!formatter.format(&file).await.unwrap());
    assert!(!formatter.format(&root.join("other.md")).await.unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn slow_formatters_time_out() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = root.join("bin").join("slow-fmt");
    executable(&script, "#!/bin/sh\nsleep 5\n");
    let config = FormatterConfig {
        builtins: false,
        custom: vec![custom("slow", script.to_str().unwrap(), &[".txt"])],
        timeout: std::time::Duration::from_millis(200),
        ..FormatterConfig::default()
    };
    let formatter = Formatter::new(root, config);
    let file = root.join("note.txt");
    fs::write(&file, "x").unwrap();
    assert!(matches!(
        formatter.format(&file).await,
        Err(harness_workspace::FormatError::Timeout { .. })
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn failing_formatters_report_their_exit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let script = root.join("bin").join("bad-fmt");
    executable(&script, "#!/bin/sh\necho broken >&2\nexit 3\n");
    let config = FormatterConfig {
        builtins: false,
        custom: vec![custom("bad", script.to_str().unwrap(), &[".txt"])],
        ..FormatterConfig::default()
    };
    let formatter = Formatter::new(root, config);
    let file = root.join("note.txt");
    fs::write(&file, "x").unwrap();
    match formatter.format(&file).await {
        Err(harness_workspace::FormatError::Failed { code, stderr, .. }) => {
            assert_eq!(code, Some(3));
            assert_eq!(stderr, "broken");
        },
        other => panic!("unexpected result: {other:?}"),
    }
}
