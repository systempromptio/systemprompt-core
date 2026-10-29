use systemprompt_bridge::install::mdm::claude_code_settings::permissions::{
    PermissionOutcome, PermissionRules, apply_permissions,
};
use systemprompt_bridge::install::mdm::claude_code_settings::standalone_settings_path;

fn rules() -> PermissionRules {
    PermissionRules {
        allow: vec!["mcp__acme__search".to_owned()],
        deny: Vec::new(),
    }
}

fn sandbox<R>(f: impl FnOnce() -> R) -> R {
    let home = tempfile::tempdir().expect("home");
    let root = home.path().display().to_string();
    temp_env::with_vars(
        [
            ("HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(format!("{root}/config"))),
            ("XDG_STATE_HOME", Some(format!("{root}/state"))),
            ("CLAUDE_CONFIG_DIR", Some(format!("{root}/claude"))),
            ("SP_BRIDGE_CONFIG", None),
        ],
        f,
    )
}

#[test]
fn rules_already_carried_do_not_warn_and_leave_the_file_untouched() {
    sandbox(|| {
        let standalone = standalone_settings_path().expect("path");
        std::fs::create_dir_all(standalone.parent().unwrap()).unwrap();
        std::fs::write(&standalone, "{}").unwrap();

        let first = apply_permissions(&rules()).expect("first apply");
        assert!(matches!(first, PermissionOutcome::Written(ref lines) if !lines.is_empty()));
        let after_first = std::fs::read(&standalone).unwrap();

        let second = apply_permissions(&rules()).expect("second apply");
        assert_eq!(
            second,
            PermissionOutcome::Written(Vec::new()),
            "no NoCarrier when already present"
        );
        assert_eq!(
            std::fs::read(&standalone).unwrap(),
            after_first,
            "byte-identical"
        );
    });
}

#[test]
fn with_no_settings_file_the_rules_still_report_no_carrier() {
    sandbox(|| {
        assert!(matches!(
            apply_permissions(&rules()).expect("apply"),
            PermissionOutcome::NoCarrier { rules: 1, .. }
        ));
    });
}
