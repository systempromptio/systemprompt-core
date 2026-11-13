//! Tests for `plugins seams`: the link-time registration listing and its
//! configured-but-not-linked check.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::plugins::seams::{
    LinkedSeam, Needed, Requirement, SeamsArgs, evaluate, execute, linked_seams,
};
use systemprompt_cli::plugins::types::SeamStatus;

fn seam(name: &'static str, entries: &[&str]) -> LinkedSeam {
    LinkedSeam {
        name,
        owner: "test-owner",
        entries: entries.iter().map(|e| (*e).to_owned()).collect(),
    }
}

fn requirement(seam: &'static str, needed: Needed) -> Requirement {
    Requirement {
        seam,
        setting: format!("{seam} setting"),
        needed,
    }
}

#[test]
fn every_seam_is_listed_once_including_empty_ones() {
    let seams = linked_seams();
    let mut names: Vec<&str> = seams.iter().map(|s| s.name).collect();
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), total, "seam names are unique");

    for expected in [
        "extensions",
        "jobs",
        "authz_hooks",
        "secrets_sources",
        "gateway_safety_scanners",
        "gateway_outbound_adapters",
        "sso_providers",
        "mcp_toolset_factories",
        "user_purge_tables",
        "retention_holds",
        "brain_filters",
        "notification_routers",
        "document_codecs",
        "cli_commands",
    ] {
        assert!(names.contains(&expected), "{expected} is enumerated");
    }
}

#[test]
fn the_compiled_binary_links_core_registrations() {
    let seams = linked_seams();
    let find = |name: &str| seams.iter().find(|s| s.name == name).unwrap();
    assert!(!find("extensions").entries.is_empty());
    assert!(!find("jobs").entries.is_empty());
    assert!(
        find("cli_commands")
            .entries
            .iter()
            .any(|entry| entry == "build")
    );
}

#[test]
fn a_seam_without_requirements_is_linked_or_empty() {
    let (rows, all_linked) = evaluate(vec![seam("a", &["x"]), seam("b", &[])], &[]);
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::Linked);
    assert_eq!(rows[0].count, 1);
    assert_eq!(rows[0].entries, "x");
    assert_eq!(rows[1].status, SeamStatus::Empty);
    assert_eq!(rows[1].requires, "");
}

#[test]
fn a_required_seam_with_no_entries_is_missing() {
    let (rows, all_linked) = evaluate(
        vec![seam("authz_hooks", &[])],
        &[requirement("authz_hooks", Needed::AnyEntry)],
    );
    assert!(!all_linked);
    assert_eq!(rows[0].status, SeamStatus::Missing);
    assert_eq!(rows[0].requires, "authz_hooks setting");
}

#[test]
fn a_required_entry_must_be_present_by_name() {
    let reqs = [requirement(
        "secrets_sources",
        Needed::Entry("vault-x".to_owned()),
    )];

    let (rows, all_linked) = evaluate(vec![seam("secrets_sources", &["other"])], &reqs);
    assert!(!all_linked);
    assert_eq!(rows[0].status, SeamStatus::Missing);

    let (rows, all_linked) = evaluate(vec![seam("secrets_sources", &["other", "vault-x"])], &reqs);
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::Linked);
}

#[test]
fn execute_without_a_profile_renders_a_table_and_passes() {
    let (out, all_linked) = execute(&SeamsArgs { seam: None }, None);
    assert!(all_linked);
    let artifact = serde_json::to_value(out.artifact()).unwrap();
    let items = artifact["items"].as_array().expect("table rows");
    assert_eq!(items.len(), linked_seams().len());
    assert!(items[0].get("status").is_some());
}

#[test]
fn the_seam_filter_narrows_rows() {
    let (out, _) = execute(
        &SeamsArgs {
            seam: Some("brain_".to_owned()),
        },
        None,
    );
    let artifact = serde_json::to_value(out.artifact()).unwrap();
    let items = artifact["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(
        items
            .iter()
            .all(|row| row["seam"].as_str().unwrap().starts_with("brain_"))
    );
}
