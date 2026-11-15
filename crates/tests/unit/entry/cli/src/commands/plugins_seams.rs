//! Tests for `plugins seams`: the link-time registration listing and its
//! configured-but-not-linked check.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::kernel::{LinkedSeamRegistration, SeamEntry, named, registered_seams};
use systemprompt_cli::plugins::seams::{
    LinkedSeam, Needed, Requirement, SeamsArgs, evaluate, execute, linked_seams,
};
use systemprompt_cli::plugins::types::SeamStatus;
use systemprompt_identifiers::ExtensionId;

systemprompt_cli::register_linked_seam!(LinkedSeamRegistration {
    name: "test_contributed_seam",
    owner: "systemprompt-cli-tests",
    extension: Some("test_capability"),
    entries: || named(["second", "first"]),
});

fn seam(name: &'static str, entries: &[&str]) -> LinkedSeam {
    owned_seam(name, None, entries)
}

fn owned_seam(name: &'static str, extension: Option<&'static str>, entries: &[&str]) -> LinkedSeam {
    LinkedSeam {
        name,
        owner: "test-owner",
        extension,
        entries: entries.iter().map(|e| SeamEntry::new(*e)).collect(),
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
            .any(|entry| entry.name == "build")
    );
}

#[test]
fn a_seam_without_requirements_is_linked_or_empty() {
    let (rows, all_linked) = evaluate(vec![seam("a", &["x"]), seam("b", &[])], &[], &[]);
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
        &[],
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

    let (rows, all_linked) = evaluate(vec![seam("secrets_sources", &["other"])], &reqs, &[]);
    assert!(!all_linked);
    assert_eq!(rows[0].status, SeamStatus::Missing);

    let (rows, all_linked) = evaluate(
        vec![seam("secrets_sources", &["other", "vault-x"])],
        &reqs,
        &[],
    );
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::Linked);
}

#[test]
fn execute_without_a_profile_renders_a_table_and_passes() {
    let (out, all_linked) = execute(
        &SeamsArgs { seam: None },
        None,
        &systemprompt_config::SecretsSources::default(),
    );
    assert!(all_linked);
    let artifact = serde_json::to_value(out.artifact()).unwrap();
    let items = artifact["items"].as_array().expect("table rows");
    assert_eq!(items.len(), linked_seams().len() + 1);
    assert!(
        items
            .iter()
            .any(|row| row["seam"] == "secrets_sources" && row["status"] == "empty")
    );
    assert!(items[0].get("status").is_some());
}

#[test]
fn the_seam_filter_narrows_rows() {
    let (out, _) = execute(
        &SeamsArgs {
            seam: Some("brain_".to_owned()),
        },
        None,
        &systemprompt_config::SecretsSources::default(),
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

#[test]
fn a_seam_registered_outside_the_kernel_is_listed_with_its_owner() {
    let seams = linked_seams();
    let contributed = seams
        .iter()
        .find(|s| s.name == "test_contributed_seam")
        .expect("a registration from another crate is listed");
    assert_eq!(contributed.owner, "systemprompt-cli-tests");
    assert_eq!(contributed.extension, Some("test_capability"));
    let names: Vec<&str> = contributed
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, vec!["first", "second"]);
    assert!(contributed.entries.iter().all(|e| e.owner.is_none()));
}

#[test]
fn registrations_are_listed_in_name_order() {
    let names: Vec<&str> = registered_seams().iter().map(|s| s.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn a_seam_whose_owner_the_profile_disables_is_marked_owner_disabled() {
    let disabled = [ExtensionId::new("brain")];
    let (rows, all_linked) = evaluate(
        vec![
            owned_seam("brain_filters", Some("brain"), &["priority=1"]),
            owned_seam("gateway_route_selectors", Some("gateway"), &["x"]),
            seam("jobs", &["y"]),
        ],
        &[],
        &disabled,
    );
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::OwnerDisabled);
    assert_eq!(rows[0].extension, "brain");
    assert_eq!(rows[1].status, SeamStatus::Linked);
    assert_eq!(rows[1].extension, "gateway");
    assert_eq!(rows[2].status, SeamStatus::Linked);
    assert_eq!(rows[2].extension, "kernel");
}

#[test]
fn a_missing_requirement_outranks_a_disabled_owner() {
    let (rows, all_linked) = evaluate(
        vec![owned_seam("authz_hooks", Some("authz"), &[])],
        &[requirement("authz_hooks", Needed::AnyEntry)],
        &[ExtensionId::new("authz")],
    );
    assert!(!all_linked);
    assert_eq!(rows[0].status, SeamStatus::Missing);
}

#[test]
fn a_required_seam_whose_owner_is_disabled_is_missing_and_fails() {
    let (rows, all_linked) = evaluate(
        vec![owned_seam(
            "secrets_sources",
            Some("vault_ext"),
            &["vault-x"],
        )],
        &[requirement(
            "secrets_sources",
            Needed::Entry("vault-x".to_owned()),
        )],
        &[ExtensionId::new("vault_ext")],
    );
    assert!(!all_linked);
    assert_eq!(rows[0].status, SeamStatus::Missing);
    assert_eq!(rows[0].requires, "secrets_sources setting");
}

#[test]
fn a_disabled_owner_nobody_requires_is_informational() {
    let (rows, all_linked) = evaluate(
        vec![owned_seam(
            "gateway_routes",
            Some("gateway"),
            &["notifications:bridge"],
        )],
        &[requirement("authz_hooks", Needed::AnyEntry)],
        &[ExtensionId::new("gateway")],
    );
    assert!(
        !all_linked,
        "the unregistered authz_hooks requirement fails"
    );
    let gateway = rows
        .iter()
        .find(|row| row.seam == "gateway_routes")
        .unwrap();
    assert_eq!(gateway.status, SeamStatus::OwnerDisabled);

    let (rows, all_linked) = evaluate(
        vec![owned_seam(
            "gateway_routes",
            Some("gateway"),
            &["notifications:bridge"],
        )],
        &[],
        &[ExtensionId::new("gateway")],
    );
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::OwnerDisabled);
}

#[test]
fn a_requirement_naming_an_unregistered_seam_is_reported_missing() {
    let (rows, all_linked) = evaluate(
        vec![seam("jobs", &["x"])],
        &[requirement("authz_hooks", Needed::AnyEntry)],
        &[],
    );
    assert!(!all_linked);
    let row = rows.iter().find(|row| row.seam == "authz_hooks").unwrap();
    assert_eq!(row.status, SeamStatus::Missing);
    assert_eq!(row.owner, "unregistered");
    assert_eq!(row.requires, "authz_hooks setting");
}

#[test]
fn the_gateway_route_seam_is_registered() {
    let seams = linked_seams();
    let routes = seams
        .iter()
        .find(|s| s.name == "gateway_routes")
        .expect("gateway route registrations are a listed seam");
    assert_eq!(routes.owner, "systemprompt-api");
    assert_eq!(routes.extension, None);
    assert!(routes.entries.iter().all(|entry| entry.owner.is_some()));
}

#[derive(Default)]
struct AdmitAllGuard;

#[async_trait::async_trait]
impl systemprompt_extension::GatewayRequestGuard for AdmitAllGuard {
    async fn check(
        &self,
        _db: &dyn systemprompt_traits::DatabaseHandle,
        _request: &systemprompt_extension::GatewayGuardRequest<'_>,
    ) -> Result<(), systemprompt_extension::GatewayDenyReason> {
        Ok(())
    }
}

systemprompt_extension::register_gateway_guard!("gateway", AdmitAllGuard);

#[test]
fn gateway_request_guards_are_listed_by_owning_extension() {
    let seams = linked_seams();
    let guards = seams
        .iter()
        .find(|s| s.name == "gateway_request_guards")
        .expect("gateway guards are a listed seam");
    assert!(!guards.entries.is_empty());
    assert_eq!(guards.extension, None);
    assert!(
        guards
            .entries
            .iter()
            .any(|entry| entry.name == "gateway" && entry.owner == Some("gateway"))
    );
    let owners: Vec<&str> = systemprompt_extension::GatewayGuards::registrations()
        .map(|registration| registration.extension)
        .collect();
    assert_eq!(guards.entries.len(), owners.len());
    assert!(
        guards
            .entries
            .iter()
            .all(|entry| entry.owner.is_some_and(|owner| owners.contains(&owner)))
    );
}

fn per_entry_seam(entries: &[(&str, &'static str)]) -> LinkedSeam {
    LinkedSeam {
        name: "gateway_routes",
        owner: "test-owner",
        extension: None,
        entries: entries
            .iter()
            .map(|(name, owner)| SeamEntry::owned_by(*name, owner))
            .collect(),
    }
}

#[test]
fn a_required_entry_is_unmet_when_its_own_owner_is_disabled() {
    let required = [requirement(
        "gateway_routes",
        Needed::Entry("bridge_notifications".to_owned()),
    )];
    let (rows, all_linked) = evaluate(
        vec![per_entry_seam(&[(
            "bridge_notifications",
            "notifications-http",
        )])],
        &required,
        &[ExtensionId::new("notifications-http")],
    );
    assert!(!all_linked);
    assert_eq!(rows[0].status, SeamStatus::Missing);
    assert!(
        rows[0].entries.contains("owner_disabled"),
        "{}",
        rows[0].entries
    );
}

#[test]
fn a_required_entry_is_met_when_its_owner_is_enabled_though_gateway_is_disabled() {
    let required = [requirement(
        "gateway_routes",
        Needed::Entry("bridge_notifications".to_owned()),
    )];
    let (rows, all_linked) = evaluate(
        vec![per_entry_seam(&[(
            "bridge_notifications",
            "notifications-http",
        )])],
        &required,
        &[ExtensionId::new("gateway")],
    );
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::Linked);
    assert!(
        !rows[0].entries.contains("owner_disabled"),
        "{}",
        rows[0].entries
    );
}

#[test]
fn each_entry_is_marked_by_its_own_owner_and_the_seam_stays_linked_while_one_is_active() {
    let (rows, all_linked) = evaluate(
        vec![per_entry_seam(&[
            ("a_scope", "ext_a"),
            ("b_scope", "ext_b"),
        ])],
        &[requirement("gateway_routes", Needed::AnyEntry)],
        &[ExtensionId::new("ext_a")],
    );
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::Linked);
    assert_eq!(
        rows[0].entries,
        "a_scope [ext_a: owner_disabled], b_scope [ext_b]"
    );

    let (rows, all_linked) = evaluate(
        vec![per_entry_seam(&[
            ("a_scope", "ext_a"),
            ("b_scope", "ext_b"),
        ])],
        &[],
        &[ExtensionId::new("ext_a"), ExtensionId::new("ext_b")],
    );
    assert!(all_linked);
    assert_eq!(rows[0].status, SeamStatus::OwnerDisabled);
}
