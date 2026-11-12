//! The environment a spawned MCP server boots a `HostContext` from, built for
//! a fixture bootstrap: the profile path, the database URL and the fixture
//! deployment's public verification keys, and nothing else.

use std::collections::BTreeMap;

use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;

use crate::bootstrap::TestBootstrap;

pub fn host_child_env(bootstrap: &TestBootstrap) -> BTreeMap<String, String> {
    let keys = crate::jwt::test_signing_keys();
    BTreeMap::from([
        (
            "SYSTEMPROMPT_PROFILE".to_owned(),
            bootstrap.profile_path.display().to_string(),
        ),
        ("DATABASE_URL".to_owned(), bootstrap.database_url.clone()),
        (
            VERIFICATION_KEYS_ENV.to_owned(),
            serde_json::to_string(keys.verification_keys()).expect("verification keys serialise"),
        ),
    ])
}

// The fixture services config with one managed server, `server`, hosting the
// given toolset ids.
pub fn services_hosting(server: &str, toolsets: &[&str]) -> systemprompt_loader::ServicesState {
    let boot = &crate::bootstrap::ensure_test_bootstrap().boot;
    let mut config = (*crate::app_context::fixture_services_state().config()).clone();
    let listed: String = toolsets
        .iter()
        .map(|id| format!("  - id: {id}\n"))
        .collect();
    let yaml = format!(
        "binary: systemprompt-mcp-fixture\nport: 5199\nenabled: true\ndisplay_in_web: \
         false\noauth:\n  required: true\n  scopes: [user]\n  audience: mcp\n  client_id: \
         null\ntoolsets:\n{listed}"
    );
    let yaml = if toolsets.is_empty() {
        yaml.replace("toolsets:\n", "")
    } else {
        yaml
    };
    config.mcp_servers.insert(
        server.to_owned(),
        serde_yaml::from_str(&yaml).expect("fixture deployment yaml"),
    );
    systemprompt_loader::ServicesState::from_config(boot.profile(), config)
        .expect("fixture services config resolves")
}
