use std::path::PathBuf;

use systemprompt_cli::commands::admin::config::migrate::profile_layer::ProfileMigration;
use systemprompt_cli::commands::admin::config::migrate::profiles::{
    LayerChange, apply, describe, plan,
};

fn change(
    source: &str,
    target: &str,
    migration: ProfileMigration,
    joins_base: bool,
) -> LayerChange {
    LayerChange {
        source: PathBuf::from(source),
        target: PathBuf::from(target),
        migration,
        joins_base,
    }
}

#[test]
fn a_directory_without_profiles_is_refused_by_plan_and_apply() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("notes.yaml"), "a: 1\n").expect("stray file");
    for err in [
        plan(dir.path()).expect_err("plan"),
        apply(dir.path()).expect_err("apply"),
    ] {
        assert!(err.to_string().contains("holds no profile"), "{err:#}");
    }
    assert!(dir.path().join("notes.yaml").exists());
}

#[test]
fn a_missing_directory_is_reported_by_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("absent");
    let err = plan(&missing).expect_err("missing dir");
    assert!(format!("{err:#}").contains("absent"), "{err:#}");
}

#[test]
fn an_overlay_that_is_not_yaml_is_refused_naming_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("base.yaml"), "site:\n  name: S\n").expect("base");
    std::fs::create_dir_all(dir.path().join("dev")).expect("mkdir");
    std::fs::write(dir.path().join("dev/profile.yaml"), "name: [unclosed\n").expect("overlay");
    let err = plan(dir.path()).expect_err("bad overlay");
    assert!(format!("{err:#}").contains("profile.yaml"), "{err:#}");
}

#[test]
fn nothing_pending_reads_as_already_migrated() {
    assert_eq!(describe(&[]), "every profile is already in the 0.73 shape");
}

#[test]
fn the_report_lists_every_kind_of_rewrite() {
    let rewritten = ProfileMigration {
        text: String::new(),
        external_url: Some("https://app.example.com".to_owned()),
        extra_cors_origins: vec!["http://a".to_owned(), "http://b".to_owned()],
        removed: vec!["server.api_server_url".to_owned()],
        renamed_judge: true,
        rooted_at_parent: true,
    };
    let report = describe(&[
        change(
            "p/prod.secrets.profile.yaml",
            "p/prod/profile.yaml",
            rewritten,
            true,
        ),
        change(
            "p/dev/profile.yaml",
            "p/dev/profile.yaml",
            ProfileMigration::default(),
            false,
        ),
    ]);
    let lines: Vec<&str> = report.lines().collect();
    assert_eq!(
        lines[0],
        "p/prod.secrets.profile.yaml -> p/prod/profile.yaml"
    );
    assert!(lines.contains(&"  server.external_url: https://app.example.com"));
    assert!(lines.contains(&"  server.extra_cors_origins: http://a, http://b"));
    assert!(lines.contains(&"  removed (now derived): server.api_server_url"));
    assert!(lines.contains(&"  evaluation: renamed to judge:"));
    assert!(lines.iter().any(|l| l.contains("now loads over base.yaml")));
    assert!(lines.iter().any(|l| l.starts_with("  paths.root")));
    assert_eq!(lines.last(), Some(&"p/dev/profile.yaml"));
}
