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

#[cfg(unix)]
#[test]
fn an_npm_cmd_shim_is_bypassed_so_node_itself_is_the_bounded_child() {
    let plugin = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    let bin = tempdir().unwrap();
    std::fs::write(
        bin.path().join("npm.cmd"),
        "@ECHO OFF\r\nnode npm-cli.js %*\r\n",
    )
    .unwrap();
    let cli = bin.path().join("node_modules/npm/bin");
    std::fs::create_dir_all(&cli).unwrap();
    std::fs::write(cli.join("npm-cli.js"), "").unwrap();
    fake_tool(
        bin.path(),
        "node",
        "mkdir -p node_modules && printf '%s\\n' \"$*\" > node_modules/args",
    );
    assert_eq!(
        with_path(bin.path(), || install(plugin.path())),
        NodeInstall::Installed { tool: "npm" }
    );
    let args = std::fs::read_to_string(plugin.path().join("node_modules/args")).unwrap();
    assert_eq!(
        args.trim(),
        format!(
            "{} ci --ignore-scripts --no-audit --no-fund",
            cli.join("npm-cli.js").display()
        )
    );
}

#[cfg(unix)]
fn installed_plugin(bin: &Path) -> TempDir {
    let plugin = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    assert_eq!(
        with_path(bin, || install(plugin.path())),
        NodeInstall::Installed { tool: "npm" }
    );
    plugin
}

#[cfg(unix)]
#[test]
fn promotion_carries_node_modules_forward_only_after_the_staged_tree_has_landed() {
    use systemprompt_bridge::sync::apply::swap::promote_staged;
    let bin = tempdir().unwrap();
    fake_tool(
        bin.path(),
        "npm",
        "mkdir -p node_modules && touch node_modules/installed",
    );
    let root = tempdir().unwrap();
    let target = root.path().join("plugin");
    std::fs::rename(installed_plugin(bin.path()).keep(), &target).unwrap();
    let staged = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    let stage = root.path().join("stage");
    std::fs::rename(staged.keep(), &stage).unwrap();

    assert!(promote_staged(&stage, &target, "plugin").unwrap());
    assert!(target.join("node_modules/installed").exists());
    assert!(!root.path().join("plugin.old").exists());
    assert_eq!(
        with_path(bin.path(), || install(&target)),
        NodeInstall::Unchanged
    );
}

#[cfg(unix)]
#[test]
fn a_failed_promotion_restores_the_installed_plugin_with_its_node_modules() {
    use systemprompt_bridge::sync::apply::swap::promote_staged;
    let bin = tempdir().unwrap();
    fake_tool(
        bin.path(),
        "npm",
        "mkdir -p node_modules && touch node_modules/installed",
    );
    let root = tempdir().unwrap();
    let target = root.path().join("plugin");
    std::fs::rename(installed_plugin(bin.path()).keep(), &target).unwrap();
    let missing_stage = root.path().join("stage-that-never-landed");

    promote_staged(&missing_stage, &target, "plugin").expect_err("promotion cannot complete");
    assert!(target.join("package.json").exists());
    assert!(
        target.join("node_modules/installed").exists(),
        "the restored plugin keeps its packages"
    );
    assert_eq!(
        with_path(bin.path(), || install(&target)),
        NodeInstall::Unchanged
    );
}

#[cfg(unix)]
#[test]
fn an_unlaunchable_installer_is_a_warning_and_never_leaves_a_reuse_stamp() {
    use std::os::unix::fs::PermissionsExt as _;

    let plugin = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    let bin = tempdir().unwrap();
    let npm = bin.path().join("npm");
    std::fs::write(&npm, "#!/definitely/not/a-shell\n").unwrap();
    std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();

    match with_path(bin.path(), || install(plugin.path())) {
        NodeInstall::Skipped { reason } => assert!(
            reason.contains("npm could not be started"),
            "the process launch failure is operator-visible: {reason}"
        ),
        other => panic!("a bad interpreter cannot install packages: {other:?}"),
    }
    assert!(
        !plugin
            .path()
            .join("node_modules/.systemprompt-install.sha256")
            .exists(),
        "a process that did not launch must not be considered reusable"
    );
}

#[cfg(unix)]
#[test]
fn a_successful_installer_with_an_unwritable_stamp_location_is_not_reused() {
    let plugin = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    let bin = tempdir().unwrap();
    fake_tool(bin.path(), "npm", ":");
    std::fs::write(plugin.path().join("node_modules"), "not a directory").unwrap();

    match with_path(bin.path(), || install(plugin.path())) {
        NodeInstall::Skipped { reason } => {
            assert!(reason.contains("finished but"), "{reason}");
            assert!(reason.contains(".systemprompt-install.sha256"), "{reason}");
        },
        other => panic!("a missing durable stamp cannot report success: {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(plugin.path().join("node_modules")).unwrap(),
        "not a directory",
        "the bridge does not replace an unexpected node_modules file"
    );
}

#[cfg(unix)]
#[test]
fn failed_node_modules_carry_over_preserves_the_installed_tree_for_recovery() {
    let bin = tempdir().unwrap();
    fake_tool(
        bin.path(),
        "npm",
        "mkdir -p node_modules && printf retained > node_modules/original",
    );
    let installed = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    assert_eq!(
        with_path(bin.path(), || install(installed.path())),
        NodeInstall::Installed { tool: "npm" }
    );
    let staged = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    std::fs::create_dir(staged.path().join("node_modules")).unwrap();
    std::fs::write(staged.path().join("node_modules/foreign"), "leave me").unwrap();

    carry_over(installed.path(), staged.path());

    assert!(
        installed.path().join("node_modules/original").is_file(),
        "a failed rename leaves the active package tree available to the installed plugin"
    );
    assert!(
        staged.path().join("node_modules/foreign").is_file(),
        "carry-over does not clobber an already-staged directory"
    );
}

#[cfg(unix)]
#[test]
fn an_installer_that_exceeds_the_deadline_is_reaped_and_never_stamped() {
    let plugin = plugin_with(&[("package.json", "{}"), ("package-lock.json", "{}")]);
    let bin = tempdir().unwrap();
    let pid_file = plugin.path().join("npm.pid");
    fake_tool(
        bin.path(),
        "npm",
        &format!("echo $$ > '{}'; exec /bin/sleep 70", pid_file.display()),
    );

    let started = std::time::Instant::now();
    match with_path(bin.path(), || install(plugin.path())) {
        NodeInstall::Skipped { reason } => assert!(
            reason.contains("exceeded the 60s install deadline") && reason.contains("was stopped"),
            "the bounded installer explains both the deadline and termination: {reason}"
        ),
        other => panic!("a timed-out installer cannot report success: {other:?}"),
    }
    assert!(
        started.elapsed() >= std::time::Duration::from_secs(60),
        "the fake installer reached the bridge deadline"
    );
    let pid = std::fs::read_to_string(&pid_file)
        .expect("installer recorded its own pid")
        .trim()
        .to_owned();
    assert!(
        !std::process::Command::new("/bin/kill")
            .args(["-0", &pid])
            .status()
            .expect("check recorded child pid")
            .success(),
        "the bridge reaped the installer process it started"
    );
    assert!(
        !plugin
            .path()
            .join("node_modules/.systemprompt-install.sha256")
            .exists(),
        "a killed installer must be retried rather than reused"
    );
}
