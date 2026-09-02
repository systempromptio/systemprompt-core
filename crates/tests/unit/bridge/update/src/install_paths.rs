use systemprompt_bridge::update::{installed_path, spawn_installed, sweep_leftovers};

#[test]
fn installed_path_resolves_to_an_existing_file() {
    let path = installed_path().expect("the running test binary is locatable");
    assert!(path.is_absolute(), "{} is not absolute", path.display());
    assert!(path.exists(), "{} does not exist", path.display());
    if cfg!(target_os = "macos") {
        let is_bundle = path.extension().is_some_and(|e| e == "app");
        assert!(is_bundle || path.is_file());
    } else {
        assert!(path.is_file(), "{} is not a file", path.display());
    }
}

#[test]
fn installed_path_is_stable_across_calls() {
    let first = installed_path().expect("locatable");
    let second = installed_path().expect("locatable");
    assert_eq!(first, second);
}

#[test]
fn sweep_leftovers_is_safe_to_call_repeatedly() {
    sweep_leftovers();
    sweep_leftovers();
    let path = installed_path().expect("locatable");
    assert!(
        path.exists(),
        "sweeping removed the install at {}",
        path.display()
    );
}

#[test]
fn spawning_a_missing_binary_is_a_relaunch_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("not-installed-here");
    let err = spawn_installed(&missing).expect_err("a missing binary cannot be relaunched");
    assert!(
        matches!(err, systemprompt_bridge::update::UpdateError::Relaunch(_)),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn spawning_an_executable_succeeds() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("tempdir").keep();
    let script = dir.join("relaunch-me");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").expect("write script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("chmod script");
    let spawned = spawn_installed(&script);
    assert!(spawned.is_ok(), "spawn failed: {:?}", spawned.err());
    std::thread::sleep(std::time::Duration::from_millis(200));
    _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn spawning_a_non_executable_file_is_a_relaunch_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plain = dir.path().join("not-executable");
    std::fs::write(&plain, b"data").expect("write file");
    let err = spawn_installed(&plain).expect_err("a non-executable file cannot be relaunched");
    assert!(
        matches!(err, systemprompt_bridge::update::UpdateError::Relaunch(_)),
        "unexpected error: {err}"
    );
}
