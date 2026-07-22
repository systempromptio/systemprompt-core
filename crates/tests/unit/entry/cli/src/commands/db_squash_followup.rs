//! Tests for the migrate-squash follow-up step builder.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::infrastructure::db::admin_squash::build_follow_up;
use systemprompt_database::SquashPlan;

fn plan() -> SquashPlan {
    SquashPlan {
        extension_id: "content".to_owned(),
        through: 4,
        baseline_name: "000_content_baseline".to_owned(),
        baseline_sql: "CREATE TABLE t();".to_owned(),
        baseline_checksum: "abc123".to_owned(),
        source_versions: vec![1, 2, 3, 4],
        already_applied_versions: vec![1, 2],
        applied: false,
    }
}

#[test]
fn dry_run_follow_up_starts_with_apply_hint() {
    let steps = build_follow_up(&plan(), Path::new("/repo/schema/000_baseline.sql"), false);

    assert_eq!(steps.len(), 4);
    assert!(steps[0].contains("Re-run with --apply"));
    assert!(steps[0].contains("/repo/schema/000_baseline.sql"));
    assert!(steps[1].contains("[1, 2, 3, 4]"));
    assert!(steps[2].contains("000_content_baseline"));
    assert!(steps[3].contains("version is > 4"));
}

#[test]
fn applied_follow_up_omits_the_apply_hint() {
    let steps = build_follow_up(&plan(), Path::new("/repo/schema/000_baseline.sql"), true);

    assert_eq!(steps.len(), 3);
    assert!(steps[0].contains("Delete the squashed source files"));
    assert!(!steps.iter().any(|s| s.contains("Re-run with --apply")));
}
