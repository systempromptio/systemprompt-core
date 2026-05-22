use std::sync::Mutex;

use async_trait::async_trait;
use systemprompt_bridge::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use systemprompt_bridge::auth::types::HelperOutput;
use systemprompt_bridge::auth::{ChainError, evaluate_chain};
use systemprompt_bridge::gateway::GatewayError;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_identifiers::SessionId;

struct StubProvider {
    name: &'static str,
    response: Mutex<Option<Result<HelperOutput, AuthError>>>,
    calls: Mutex<usize>,
}

impl StubProvider {
    fn new(name: &'static str, response: Result<HelperOutput, AuthError>) -> Self {
        Self {
            name,
            response: Mutex::new(Some(response)),
            calls: Mutex::new(0),
        }
    }

    fn call_count(&self) -> usize {
        *self.calls.lock().expect("calls lock")
    }
}

#[async_trait]
impl AuthProvider for StubProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn authenticate(&self, _session_id: &SessionId) -> Result<HelperOutput, AuthError> {
        let mut calls = self.calls.lock().expect("calls lock");
        *calls += 1;
        drop(calls);
        self.response
            .lock()
            .expect("response lock")
            .take()
            .expect("StubProvider authenticate called more than once")
    }
}

fn ok_token() -> HelperOutput {
    HelperOutput {
        token: BearerToken::new("stub"),
        ttl: 3600,
        headers: Default::default(),
    }
}

async fn make_reqwest_error() -> reqwest::Error {
    reqwest::get("http://127.0.0.1:1/__definitely_not_listening__")
        .await
        .expect_err("connection refused yields a reqwest error")
}

async fn transient_mtls_failure() -> AuthError {
    AuthError::Failed {
        provider: "mtls",
        source: AuthFailedSource::Gateway(GatewayError::HealthCheck(Box::new(
            make_reqwest_error().await,
        ))),
    }
}

#[tokio::test]
async fn transient_failure_on_preferred_mtls_does_not_fall_through_to_pat() {
    let mtls = StubProvider::new("mtls", Err(transient_mtls_failure().await));
    let pat = StubProvider::new("pat", Ok(ok_token()));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let err = evaluate_chain(&chain, Some("mtls"), &SessionId::generate())
        .await
        .expect_err("must short-circuit");

    assert!(
        matches!(
            err,
            ChainError::PreferredTransient {
                provider: "mtls",
                ..
            }
        ),
        "expected PreferredTransient mtls, got: {err:?}",
    );
    assert_eq!(mtls.call_count(), 1);
    assert_eq!(
        pat.call_count(),
        0,
        "PAT must not be tried after preferred mtls hits a transient failure",
    );
}

#[tokio::test]
async fn terminal_failure_on_preferred_falls_through() {
    let mtls = StubProvider::new(
        "mtls",
        Err(AuthError::Failed {
            provider: "mtls",
            source: AuthFailedSource::Gateway(GatewayError::PubkeyMissing),
        }),
    );
    let pat = StubProvider::new("pat", Ok(ok_token()));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let token = evaluate_chain(&chain, Some("mtls"), &SessionId::generate())
        .await
        .expect("must fall through to PAT");
    assert_eq!(token.token.expose(), "stub");
    assert_eq!(pat.call_count(), 1);
}

#[tokio::test]
async fn transient_failure_on_non_preferred_falls_through() {
    let mtls = StubProvider::new("mtls", Err(transient_mtls_failure().await));
    let pat = StubProvider::new("pat", Ok(ok_token()));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let token = evaluate_chain(&chain, None, &SessionId::generate())
        .await
        .expect("transient on non-preferred must fall through");
    assert_eq!(token.token.expose(), "stub");
    assert_eq!(pat.call_count(), 1);
}

#[tokio::test]
async fn no_provider_succeeds_yields_none_succeeded() {
    let mtls = StubProvider::new("mtls", Err(AuthError::NotConfigured));
    let pat = StubProvider::new("pat", Err(AuthError::NotConfigured));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let err = evaluate_chain(&chain, None, &SessionId::generate())
        .await
        .expect_err("nothing configured");
    assert!(matches!(err, ChainError::NoneSucceeded));
}

#[tokio::test]
async fn is_terminal_classifies_gateway_variants() {
    assert!(AuthFailedSource::Gateway(GatewayError::PubkeyMissing).is_terminal());
    assert!(AuthFailedSource::Gateway(GatewayError::UnsafePath("..".into())).is_terminal());
    let transient = AuthFailedSource::Gateway(GatewayError::HealthCheck(Box::new(
        make_reqwest_error().await,
    )));
    assert!(!transient.is_terminal());
}
