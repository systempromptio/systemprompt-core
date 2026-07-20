use async_trait::async_trait;
use systemprompt_bridge::auth::provider_chain;
use systemprompt_bridge::auth::providers::{AuthError, AuthProvider};
use systemprompt_bridge::auth::types::HelperOutput;
use systemprompt_bridge::config::Config;
use systemprompt_bridge::register_auth_provider;
use systemprompt_identifiers::SessionId;

struct TestProvider;

#[async_trait]
impl AuthProvider for TestProvider {
    fn name(&self) -> &'static str {
        "test-provider"
    }
    async fn authenticate(&self, _session_id: &SessionId) -> Result<HelperOutput, AuthError> {
        Err(AuthError::NotConfigured)
    }
}

register_auth_provider!(|_cfg| Box::new(TestProvider), priority = 100);

#[test]
fn provider_chain_is_priority_ordered() {
    let names: Vec<&str> = provider_chain(&Config::default())
        .iter()
        .map(|p| p.name())
        .collect();

    assert_eq!(
        names.first().copied(),
        Some("test-provider"),
        "priority-100 provider must run first; chain = {names:?}"
    );

    let builtins: Vec<&str> = names
        .into_iter()
        .filter(|n| *n != "test-provider")
        .collect();
    assert_eq!(
        builtins,
        vec!["mtls", "session", "pat"],
        "built-in providers must stay ordered mtls > session > pat"
    );
}
