//! The bridge-side Node install: it runs only for a plugin root that carries
//! both `package.json` and a lockfile Claude Code would accept, invokes the
//! installer exactly as Claude Code does, never fails the sync, and does not
//! repeat itself for an unchanged plugin.

use std::path::Path;

use systemprompt_bridge::sync::apply::node_deps::{
    NodeInstall, binary_on_path, carry_over, install,
};
use tempfile::{TempDir, tempdir};

fn plugin_with(files: &[(&str, &str)]) -> TempDir {
    let dir = tempdir().unwrap();
    for (name, body) in files {
        std::fs::write(dir.path().join(name), body).unwrap();
    }
    dir
}

fn with_path<T>(dir: &Path, body: impl FnOnce() -> T) -> T {
    temp_env::with_var("PATH", Some(dir.as_os_str()), body)
}

#[test]
fn install_is_not_applicable_without_package_json_and_lockfile() {
    let empty = tempdir().unwrap();
    assert_eq!(install(empty.path()), NodeInstall::NotApplicable);

    let no_lock = plugin_with(&[("package.json", "{}")]);
    assert_eq!(install(no_lock.path()), NodeInstall::NotApplicable);

    let yarn = plugin_with(&[("package.json", "{}"), ("yarn.lock", "")]);
    assert_eq!(
        install(yarn.path()),
        NodeInstall::NotApplicable,
        "yarn cannot be told to skip lifecycle scripts, so Claude Code skips it and so do we"
    );
}

#[test]
fn a_missing_installer_is_a_warning_not_an_error() {
    let plugin = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    let empty_path = tempdir().unwrap();
    let outcome = with_path(empty_path.path(), || install(plugin.path()));
    match outcome {
        NodeInstall::Skipped { reason } => {
            assert!(reason.contains("npm is not on PATH"), "{reason}")
        },
        other => panic!("expected a skipped install, got {other:?}"),
    }
}

#[cfg(unix)]
fn fake_tool(bin_dir: &Path, name: &str, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = bin_dir.join(name);
    std::fs::write(
        &path,
        format!("#!/bin/sh\nPATH=/usr/bin:/bin:$PATH\n{script}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn npm_ci_runs_with_ignore_scripts_in_the_plugin_dir_and_is_not_repeated() {
    let plugin = plugin_with(&[
        ("package.json", r#"{"name":"app"}"#),
        ("package-lock.json", r#"{"lockfileVersion":3}"#),
    ]);
    let bin = tempdir().unwrap();
    fake_tool(
        bin.path(),
        "npm",
        "mkdir -p node_modules && printf '%s\\n' \"$*\" > node_modules/args && pwd > node_modules/cwd",
    );

    let first = with_path(bin.path(), || install(plugin.path()));
    assert_eq!(first, NodeInstall::Installed { tool: "npm" });
    let args = std::fs::read_to_string(plugin.path().join("node_modules/args")).unwrap();
    assert_eq!(args.trim(), "ci --ignore-scripts --no-audit --no-fund");
    let cwd = std::fs::read_to_string(plugin.path().join("node_modules/cwd")).unwrap();
    assert_eq!(
        Path::new(cwd.trim()).canonicalize().unwrap(),
        plugin.path().canonicalize().unwrap()
    );

    let second = with_path(bin.path(), || install(plugin.path()));
    assert_eq!(second, NodeInstall::Unchanged);

    std::fs::write(
        plugin.path().join("package-lock.json"),
        r#"{"lockfileVersion":4}"#,
    )
    .unwrap();
    let third = with_path(bin.path(), || install(plugin.path()));
    assert_eq!(
        third,
        NodeInstall::Installed { tool: "npm" },
        "a changed lockfile reinstalls"
    );
}

#[cfg(unix)]
#[test]
fn bun_lockfiles_use_bun_with_a_frozen_lockfile() {
    let plugin = plugin_with(&[("package.json", "{}"), ("bun.lock", "")]);
    let bin = tempdir().unwrap();
    fake_tool(
        bin.path(),
        "bun",
        "mkdir -p node_modules && printf '%s\\n' \"$*\" > node_modules/args",
    );
    assert_eq!(
        with_path(bin.path(), || install(plugin.path())),
        NodeInstall::Installed { tool: "bun" }
    );
    let args = std::fs::read_to_string(plugin.path().join("node_modules/args")).unwrap();
    assert_eq!(args.trim(), "install --frozen-lockfile --ignore-scripts");
}

#[cfg(unix)]
#[test]
fn a_failing_installer_reports_its_stderr_tail_as_a_warning() {
    let plugin = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    let bin = tempdir().unwrap();
    fake_tool(
        bin.path(),
        "npm",
        "echo 'ERESOLVE could not resolve' >&2; exit 1",
    );
    match with_path(bin.path(), || install(plugin.path())) {
        NodeInstall::Skipped { reason } => {
            assert!(reason.contains("npm exited with"), "{reason}");
            assert!(reason.contains("ERESOLVE could not resolve"), "{reason}");
        },
        other => panic!("expected a skipped install, got {other:?}"),
    }
    assert!(
        !plugin
            .path()
            .join("node_modules")
            .join(".systemprompt-install.sha256")
            .exists(),
        "a failed install leaves no stamp, so the next sync retries"
    );
}

#[cfg(unix)]
#[test]
fn carry_over_moves_node_modules_only_when_the_package_files_are_identical() {
    let bin = tempdir().unwrap();
    fake_tool(
        bin.path(),
        "npm",
        "mkdir -p node_modules && touch node_modules/installed",
    );
    let installed = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    assert_eq!(
        with_path(bin.path(), || install(installed.path())),
        NodeInstall::Installed { tool: "npm" }
    );

    let same = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    carry_over(installed.path(), same.path());
    assert!(same.path().join("node_modules/installed").exists());
    assert!(!installed.path().join("node_modules").exists());
    assert_eq!(
        with_path(bin.path(), || install(same.path())),
        NodeInstall::Unchanged
    );

    let changed = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{\"v\":2}")]);
    carry_over(same.path(), changed.path());
    assert!(
        !changed.path().join("node_modules").exists(),
        "a different lockfile must not inherit the old install"
    );
    assert!(same.path().join("node_modules").exists());
}

#[test]
fn binary_on_path_finds_only_files_in_path_entries() {
    let bin = tempdir().unwrap();
    std::fs::write(bin.path().join("present"), "").unwrap();
    std::fs::create_dir(bin.path().join("a-dir")).unwrap();
    with_path(bin.path(), || {
        assert_eq!(binary_on_path("present"), Some(bin.path().join("present")));
        assert_eq!(binary_on_path("a-dir"), None);
        assert_eq!(binary_on_path("absent"), None);
    });
}
