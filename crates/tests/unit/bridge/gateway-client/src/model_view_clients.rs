//! Per-host model visibility comes from each provider's catalog `clients`.

use std::collections::BTreeMap;

use systemprompt_bridge::gateway::model_view::{
    HostClients, HostModelView, catalog_models, claude_models, host_client, host_model_view,
};
use systemprompt_bridge::gateway::types::ProviderHealth;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::providers::ClientFamily;

fn ph(name: &str, clients: &[ClientFamily], configured: bool, models: &[&str]) -> ProviderHealth {
    ProviderHealth {
        name: name.to_owned(),
        credential: format!("{name}-key"),
        clients: clients.to_vec(),
        configured,
        models: models.iter().map(|s| (*s).to_owned()).collect(),
        config_issue: (!configured).then(|| "missing".to_owned()),
        display_name: None,
        personal_key: false,
    }
}

fn catalog() -> Vec<ProviderHealth> {
    vec![
        ph(
            "anthropic",
            &[ClientFamily::Claude, ClientFamily::OpenAi],
            true,
            &["claude-sonnet-5"],
        ),
        ph(
            "fable",
            &[ClientFamily::Claude],
            true,
            &["claude-fable-5-1"],
        ),
        ph("openai", &[ClientFamily::OpenAi], true, &["gpt-5"]),
        ph("gemini", &[ClientFamily::Gemini], true, &["gemini-3-pro"]),
        ph("hidden", &[], true, &["internal-judge"]),
    ]
}

fn models_for(host: HostKind) -> Vec<String> {
    let clients = HostClients::for_host(host, &BTreeMap::new());
    host_model_view(&catalog(), &clients.families).compatible_models
}

#[test]
fn each_host_maps_to_its_client_family() {
    assert_eq!(host_client(HostKind::ClaudeCode), ClientFamily::Claude);
    assert_eq!(host_client(HostKind::ClaudeDesktop), ClientFamily::Claude);
    assert_eq!(host_client(HostKind::CodexCli), ClientFamily::OpenAi);
    assert_eq!(host_client(HostKind::OpenCode), ClientFamily::OpenAi);
    assert_eq!(host_client(HostKind::Hermes), ClientFamily::OpenAi);
}

#[test]
fn claude_hosts_see_models_advertised_to_claude_fable_included() {
    let expected = vec!["claude-sonnet-5".to_owned(), "claude-fable-5-1".to_owned()];
    assert_eq!(models_for(HostKind::ClaudeCode), expected);
    assert_eq!(
        models_for(HostKind::ClaudeDesktop),
        expected,
        "Desktop follows the catalog, not a claude-* id predicate"
    );
}

#[test]
fn openai_hosts_see_models_advertised_to_openai() {
    let expected = vec!["claude-sonnet-5".to_owned(), "gpt-5".to_owned()];
    assert_eq!(models_for(HostKind::OpenCode), expected);
    assert_eq!(models_for(HostKind::CodexCli), expected);
}

#[test]
fn a_model_with_no_clients_is_hidden_everywhere() {
    for host in HostKind::ALL {
        assert!(
            !models_for(host).contains(&"internal-judge".to_owned()),
            "{host:?} must not list a clients: [] model"
        );
    }
    assert!(
        catalog_models(&catalog())
            .iter()
            .all(|g| g.name != "hidden"),
        "the Status card does not list it either"
    );
}

#[test]
fn a_server_filter_replaces_the_host_family() {
    let filters = BTreeMap::from([(
        HostKind::OpenCode,
        vec!["gemini".to_owned(), "not-a-family".to_owned()],
    )]);
    let clients = HostClients::for_host(HostKind::OpenCode, &filters);
    assert!(clients.filtered);
    assert_eq!(clients.families, vec![ClientFamily::Gemini]);
    assert_eq!(clients.tags(), vec!["gemini".to_owned()]);
    assert_eq!(
        host_model_view(&catalog(), &clients.families).compatible_models,
        vec!["gemini-3-pro".to_owned()]
    );
    assert!(!HostClients::for_host(HostKind::ClaudeCode, &filters).filtered);
}

#[test]
fn unconfigured_provider_is_flagged_only_for_hosts_it_advertises_to() {
    let health = vec![ph(
        "anthropic",
        &[ClientFamily::Claude],
        false,
        &["claude-sonnet-5"],
    )];
    let claude = host_model_view(&health, &[ClientFamily::Claude]);
    assert!(claude.checked && !claude.available);
    assert_eq!(claude.unconfigured_providers, vec!["anthropic".to_owned()]);
    let openai = host_model_view(&health, &[ClientFamily::OpenAi]);
    assert!(openai.unconfigured_providers.is_empty());
}

#[test]
fn unchecked_when_no_health() {
    assert_eq!(
        host_model_view(&[], &[ClientFamily::Claude]),
        HostModelView::default()
    );
}

#[test]
fn desktop_subset_of_the_status_card_is_the_claude_client_set() {
    assert_eq!(
        claude_models(&catalog_models(&catalog())),
        vec!["claude-sonnet-5".to_owned(), "claude-fable-5-1".to_owned()]
    );
}
