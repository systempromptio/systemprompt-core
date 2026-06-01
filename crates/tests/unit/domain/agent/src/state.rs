use std::sync::Arc;
use systemprompt_agent::AgentState;
use systemprompt_test_fixtures::{fixture_config, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};

struct AlwaysErrJwtProvider;

impl JwtValidationProvider for AlwaysErrJwtProvider {
    fn validate_token(&self, _token: &str) -> JwtResult<AgentJwtClaims> {
        Err(JwtProviderError::InvalidToken)
    }
    fn generate_token(&self, _params: GenerateTokenParams) -> JwtResult<String> {
        Ok("tok".to_string())
    }
    fn generate_secure_token(&self, prefix: &str) -> String {
        format!("{prefix}-fake")
    }
}

fn stub_jwt() -> systemprompt_traits::DynJwtValidationProvider {
    Arc::new(AlwaysErrJwtProvider)
}

async fn make_state() -> AgentState {
    let url = fixture_database_url().expect("DATABASE_URL");
    let pool = fixture_db_pool(&url).await.expect("pool");
    let config = Arc::new(fixture_config(&url));
    AgentState::new(pool, config, stub_jwt())
}

#[tokio::test]
async fn agent_state_new_optional_fields_are_none() {
    let state = make_state().await;
    assert!(state.user_provider().is_none());
    assert!(state.analytics_provider().is_none());
    assert!(state.session_analytics_provider().is_none());
    assert!(state.file_upload_provider().is_none());
    assert!(state.mcp_service_provider().is_none());
    assert!(state.process_cleanup_provider().is_none());
}

#[tokio::test]
async fn agent_state_config_accessor() {
    let state = make_state().await;
    let _ = state.config();
}

#[tokio::test]
async fn agent_state_jwt_provider_accessor() {
    let state = make_state().await;
    let _ = state.jwt_provider();
}

#[tokio::test]
async fn agent_state_db_pool_accessor() {
    let state = make_state().await;
    let _ = state.db_pool();
}

#[tokio::test]
async fn agent_state_debug_format() {
    let state = make_state().await;
    let debug_str = format!("{:?}", state);
    assert!(debug_str.contains("AgentState"));
    assert!(debug_str.contains("false"));
}

#[tokio::test]
async fn agent_state_clone() {
    let state = make_state().await;
    let cloned = state.clone();
    assert!(cloned.user_provider().is_none());
}
