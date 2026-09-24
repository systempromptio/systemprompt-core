//! The doctor's `OpenCode` admin-tier drift verdict: a managed file whose model
//! list no longer matches the gateway catalogue is named, with both sides of
//! the difference.

use std::path::Path;

use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::opencode::check_model_drift;

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_matching_admin_tier_is_ok_regardless_of_order() {
    let check = check_model_drift(
        Path::new("/etc/opencode/opencode.json"),
        &ids(&["b", "a"]),
        &ids(&["a", "b"]),
    );
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
}

#[test]
fn a_drifted_admin_tier_names_retired_and_missing_models() {
    let check = check_model_drift(
        Path::new("/etc/opencode/opencode.json"),
        &ids(&["claude-sonnet-4-5", "claude-opus-5-5"]),
        &ids(&["claude-opus-5-5", "claude-sonnet-5"]),
    );
    assert_eq!(check.status, Status::Warn);
    assert!(check.detail.contains("no longer served: claude-sonnet-4-5"), "{}", check.detail);
    assert!(check.detail.contains("not listed: claude-sonnet-5"), "{}", check.detail);
    assert!(check.detail.contains("as administrator"), "{}", check.detail);
}
