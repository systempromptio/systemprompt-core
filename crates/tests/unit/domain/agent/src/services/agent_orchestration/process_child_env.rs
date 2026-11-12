//! The final environment an agent child is spawned with carries the database
//! URLs, its grants and the public verification keys, and none of the
//! API-held secrets the parent process holds.

use std::collections::HashMap;
use std::sync::OnceLock;

use systemprompt_agent::services::agent_orchestration::process::command::{
    AgentEnvironmentParams, build_agent_environment,
};
use systemprompt_agent::services::agent_orchestration::process::{
    SpawnEnvironment, agent_child_secrets,
};
use systemprompt_identifiers::AgentName;
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_test_fixtures::{TestBootstrap, init_services_bootstrap, test_signing_keys};

const SERVICES: &str = r#"agents:
  env_agent:
    name: env_agent
    port: 9471
    endpoint: http://127.0.0.1:9471
    enabled: true
    card:
      protocolVersion: "0.3.0"
      displayName: Env Agent
      description: Agent whose spawn environment is inspected.
      version: "1.0.0"
    metadata: {}
    oauth:
      required: false
"#;

const API_HELD: &[&str] = &[
    "OAUTH_AT_REST_PEPPER",
    "MANIFEST_SIGNING_SECRET_SEED",
    "SIGNING_KEY_PEM",
    "SIGNING_KEY_STAGED_PEM",
    "SIGNING_KEY_PREVIOUS_KEYS",
    "ENCRYPTION_MASTER_KEY",
    "LICENCE_SIGNING_SECRET",
];

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn bootstrap() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_services_bootstrap(SERVICES))
}

#[test]
fn an_agent_child_is_spawned_without_any_api_held_secret() {
    let bootstrap = bootstrap();
    let parent = bootstrap.boot.secrets().get();
    assert!(
        !parent.oauth_at_rest_pepper.is_empty(),
        "the parent holds a pepper"
    );
    let environment = SpawnEnvironment {
        config: std::sync::Arc::clone(bootstrap.boot_config()),
        secrets: std::sync::Arc::clone(bootstrap.boot.secrets()),
        verification_keys: test_signing_keys().verification_keys().clone(),
        services: bootstrap.services.clone(),
        profile_path: bootstrap.profile_path.clone(),
    };

    let secrets_env =
        agent_child_secrets(&environment, &AgentName::new("env_agent")).expect("agent env");
    let params = AgentEnvironmentParams {
        port: 9471,
        profile_path: "/profile.yaml",
        database_type: "postgres",
        secrets_env: &secrets_env,
    };
    let env: HashMap<String, String> =
        build_agent_environment(&params, |name| std::env::var(name).ok())
            .into_iter()
            .collect();

    for name in API_HELD {
        assert!(!env.contains_key(*name), "{name} reached an agent child");
    }
    assert!(
        env.values()
            .all(|value| *value != parent.oauth_at_rest_pepper),
        "the pepper rode under another name"
    );
    let keys: serde_json::Value =
        serde_json::from_str(env.get(VERIFICATION_KEYS_ENV).expect("verification keys"))
            .expect("a verification key set");
    assert_eq!(
        keys,
        serde_json::to_value(test_signing_keys().verification_keys()).expect("serialise")
    );
    assert!(env.contains_key("DATABASE_URL"));
}
