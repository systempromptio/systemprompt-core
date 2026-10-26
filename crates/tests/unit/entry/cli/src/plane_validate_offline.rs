//! `admin config validate`'s config-plane pass: every plane is reported under
//! its own section, and a targeted validation skips the pass entirely.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::plane_validate::{
    append_full_validation, plane_declaration_results,
};

#[test]
fn every_plane_result_is_attributed_to_a_plane_section() {
    let config = systemprompt_test_fixtures::test_config();
    let results = plane_declaration_results(&config);
    assert!(
        results
            .iter()
            .all(|r| r.section.starts_with("plane:") || r.section == "config_planes"),
        "{:?}",
        results.iter().map(|r| &r.section).collect::<Vec<_>>()
    );
    assert!(results.iter().all(|r| r.valid || r.error.is_some()));
}

#[test]
fn a_targeted_validation_skips_the_plane_pass() {
    let config = systemprompt_test_fixtures::test_config();
    let mut results = Vec::new();
    let target = "web".to_owned();
    assert!(append_full_validation(&config, Some(&target), &mut results));
    assert!(results.is_empty());
}

#[test]
fn a_full_validation_appends_exactly_the_plane_results_and_reports_their_validity() {
    let config = systemprompt_test_fixtures::test_config();
    let expected = plane_declaration_results(&config);
    let mut results = Vec::new();
    let valid = append_full_validation(&config, None, &mut results);
    assert_eq!(results.len(), expected.len());
    assert_eq!(valid, expected.iter().all(|r| r.valid));
}
