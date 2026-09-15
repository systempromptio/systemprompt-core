//! Resolving the Claude Code settings path probes whether the system policy
//! dir is writable; the probe must never leave an empty `managed-settings.json`
//! behind, because Claude Code reads an empty file as an applied `{}` policy.

#![cfg(target_os = "linux")]

use systemprompt_bridge::config::paths::claude_code_policy_dir;
use systemprompt_bridge::install::mdm::claude_code_settings::managed_settings_path;
use tempfile::TempDir;

#[test]
fn probing_the_system_policy_dir_creates_no_settings_file() {
    let system = claude_code_policy_dir().join("managed-settings.json");
    let existed_before = system.exists();
    let home = TempDir::new().expect("home");
    let resolved = temp_env::with_var("HOME", Some(home.path().as_os_str()), || {
        managed_settings_path().expect("a settings path resolves with HOME set")
    });

    assert_eq!(
        system.exists(),
        existed_before,
        "the writability probe left {} behind",
        system.display()
    );
    if !resolved.starts_with(claude_code_policy_dir()) {
        assert_eq!(
            resolved,
            home.path().join(".claude").join("settings.json"),
            "without a writable system dir the per-user file is used"
        );
    }
    assert!(
        !resolved.exists(),
        "resolving the path writes nothing: {}",
        resolved.display()
    );
}
