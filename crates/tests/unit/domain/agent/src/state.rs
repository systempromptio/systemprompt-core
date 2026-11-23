use std::sync::Arc;
use systemprompt_agent::AgentState;
use systemprompt_test_fixtures::{fixture_config, test_database_url, test_db_pool};

async fn make_state() -> AgentState {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let config = Arc::new(fixture_config(&url));
    let repos = systemprompt_test_fixtures::a2a_repositories(&pool);
    let authenticator = systemprompt_test_fixtures::fixture_request_authenticator(&pool);
    AgentState::new(
        pool,
        config,
        systemprompt_agent::AgentAuth {
            authenticator,
            verification_keys: systemprompt_test_fixtures::test_signing_keys()
                .keyring()
                .clone(),
        },
        Arc::new(repos),
        systemprompt_test_mocks::recording_webhooks(),
        systemprompt_test_fixtures::fixture_services_view(),
    )
}

#[tokio::test]
async fn agent_state_config_accessor() {
    let state = make_state().await;
    let _ = state.config();
}

#[tokio::test]
async fn agent_state_authenticator_accessor() {
    let state = make_state().await;
    let _ = state.authenticator();
}

#[tokio::test]
async fn agent_state_db_pool_accessor() {
    let state = make_state().await;
    let _ = state.db_pool();
}


#[tokio::test]
async fn agent_state_clone() {
    let state = make_state().await;
    let cloned = state.clone();
    let _ = cloned.db_pool();
}
