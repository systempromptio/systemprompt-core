//! Tests for the `cloud` interactive prompts, driven through `ScriptedPrompter`
//! without touching the filesystem, cloud API, or database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::collect_api_keys;
use systemprompt_cli::cloud::profile::create_tenant::{select_tenant, select_tenant_type};
use systemprompt_cli::cloud::tenant::{
    TenantCommands, choose_tenant_operation, select_tenant as select_tenant_menu,
};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cloud::{StoredTenant, TenantStore, TenantType};
use systemprompt_identifiers::TenantId;

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn local_tenant(id: &str, name: &str) -> StoredTenant {
    StoredTenant::new_local(
        TenantId::new(id),
        name.to_owned(),
        "postgres://localhost/db".to_owned(),
    )
}

fn cloud_tenant(id: &str, name: &str) -> StoredTenant {
    let mut tenant = StoredTenant::new(TenantId::new(id), name.to_owned());
    tenant.tenant_type = TenantType::Cloud;
    tenant
}

#[test]
fn choose_tenant_operation_create() {
    let cmd = choose_tenant_operation(&scripted(&["0"]), true).expect("selection");
    assert!(matches!(cmd, Some(TenantCommands::Create { .. })));
}

#[test]
fn choose_tenant_operation_list() {
    let cmd = choose_tenant_operation(&scripted(&["1"]), true).expect("selection");
    assert!(matches!(cmd, Some(TenantCommands::List)));
}

#[test]
fn choose_tenant_operation_edit_with_tenants() {
    let cmd = choose_tenant_operation(&scripted(&["2"]), true).expect("selection");
    assert!(matches!(cmd, Some(TenantCommands::Edit { id: None })));
}

#[test]
fn choose_tenant_operation_delete_with_tenants() {
    let cmd = choose_tenant_operation(&scripted(&["3"]), true).expect("selection");
    assert!(matches!(cmd, Some(TenantCommands::Delete(_))));
}

#[test]
fn choose_tenant_operation_done_returns_none() {
    let cmd = choose_tenant_operation(&scripted(&["4"]), true).expect("selection");
    assert!(cmd.is_none());
}

#[test]
fn choose_tenant_operation_edit_without_tenants_falls_back_to_list() {
    let cmd = choose_tenant_operation(&scripted(&["2"]), false).expect("selection");
    assert!(matches!(cmd, Some(TenantCommands::List)));
}

#[test]
fn choose_tenant_operation_exhausted_errors() {
    let err = choose_tenant_operation(&scripted(&[]), true).expect_err("no answer");
    assert!(err.to_string().contains("exhausted"));
}

#[test]
fn collect_api_keys_selects_gemini() {
    let keys = collect_api_keys(&scripted(&["0", "gem-key"])).expect("keys");
    assert_eq!(keys.gemini.as_deref(), Some("gem-key"));
    assert!(keys.anthropic.is_none());
    assert!(keys.openai.is_none());
    assert_eq!(keys.selected_provider(), "gemini");
}

#[test]
fn collect_api_keys_selects_anthropic() {
    let keys = collect_api_keys(&scripted(&["1", "ant-key"])).expect("keys");
    assert_eq!(keys.anthropic.as_deref(), Some("ant-key"));
    assert_eq!(keys.selected_provider(), "anthropic");
}

#[test]
fn collect_api_keys_selects_openai() {
    let keys = collect_api_keys(&scripted(&["2", "oai-key"])).expect("keys");
    assert_eq!(keys.openai.as_deref(), Some("oai-key"));
    assert_eq!(keys.selected_provider(), "openai");
}

#[test]
fn collect_api_keys_empty_key_errors() {
    let err = collect_api_keys(&scripted(&["0", ""])).expect_err("empty key rejected");
    assert!(err.to_string().contains("required"));
}

#[test]
fn collect_api_keys_selection_out_of_range_errors() {
    let err = collect_api_keys(&scripted(&["9"])).expect_err("bad index");
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn select_tenant_menu_picks_by_index() {
    let tenants = vec![local_tenant("a", "alpha"), local_tenant("b", "beta")];
    let picked = select_tenant_menu(&scripted(&["1"]), &tenants).expect("pick");
    assert_eq!(picked.name, "beta");
}

#[test]
fn select_tenant_menu_out_of_range_errors() {
    let tenants = vec![local_tenant("a", "alpha")];
    let err = select_tenant_menu(&scripted(&["5"]), &tenants).expect_err("out of range");
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn select_tenant_type_local_when_available() {
    let store = TenantStore::new(vec![local_tenant("a", "alpha")]);
    let chosen = select_tenant_type(&scripted(&["0"]), &store).expect("local");
    assert_eq!(chosen, TenantType::Local);
}

#[test]
fn select_tenant_type_cloud_when_available() {
    let store = TenantStore::new(vec![cloud_tenant("c", "cloudy")]);
    let chosen = select_tenant_type(&scripted(&["1"]), &store).expect("cloud");
    assert_eq!(chosen, TenantType::Cloud);
}

#[test]
fn select_tenant_type_local_without_local_tenants_errors() {
    let store = TenantStore::new(vec![cloud_tenant("c", "cloudy")]);
    let err = select_tenant_type(&scripted(&["0"]), &store).expect_err("no local");
    assert!(err.to_string().contains("No local tenants"));
}

#[test]
fn select_profile_tenant_empty_errors() {
    let err = select_tenant(&scripted(&["0"]), &[]).expect_err("no tenants");
    assert!(err.to_string().contains("No eligible tenants"));
}

#[test]
fn select_profile_tenant_picks_clone() {
    let tenants = vec![local_tenant("a", "alpha"), local_tenant("b", "beta")];
    let picked = select_tenant(&scripted(&["0"]), &tenants).expect("pick");
    assert_eq!(picked.name, "alpha");
}
