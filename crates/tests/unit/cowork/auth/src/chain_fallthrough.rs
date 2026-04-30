use std::cell::Cell;

use systemprompt_cowork::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use systemprompt_cowork::auth::types::HelperOutput;
use systemprompt_cowork::auth::{ChainError, evaluate_chain};
use systemprompt_cowork::gateway::GatewayError;
use systemprompt_cowork::ids::BearerToken;

struct StubProvider {
    name: &'static str,
    response: Cell<Option<Result<HelperOutput, AuthError>>>,
    calls: Cell<usize>,
}

impl StubProvider {
    fn new(name: &'static str, response: Result<HelperOutput, AuthError>) -> Self {
        Self {
            name,
            response: Cell::new(Some(response)),
            calls: Cell::new(0),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.get()
    }
}

impl AuthProvider for StubProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn authenticate(&self) -> Result<HelperOutput, AuthError> {
        self.calls.set(self.calls.get() + 1);
        self.response
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

fn transient_mtls_failure() -> AuthError {
    AuthError::Failed {
        provider: "mtls",
        source: AuthFailedSource::Gateway(GatewayError::HealthCheck(Box::new(
            ureq::Error::Status(503, ureq::Response::new(503, "x", "x").unwrap()),
        ))),
    }
}

#[test]
fn transient_failure_on_preferred_mtls_does_not_fall_through_to_pat() {
    let mtls = StubProvider::new("mtls", Err(transient_mtls_failure()));
    let pat = StubProvider::new("pat", Ok(ok_token()));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let err = evaluate_chain(&chain, Some("mtls")).expect_err("must short-circuit");

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

#[test]
fn terminal_failure_on_preferred_falls_through() {
    let mtls = StubProvider::new(
        "mtls",
        Err(AuthError::Failed {
            provider: "mtls",
            source: AuthFailedSource::Gateway(GatewayError::PubkeyMissing),
        }),
    );
    let pat = StubProvider::new("pat", Ok(ok_token()));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let token = evaluate_chain(&chain, Some("mtls")).expect("must fall through to PAT");
    assert_eq!(token.token.expose(), "stub");
    assert_eq!(pat.call_count(), 1);
}

#[test]
fn transient_failure_on_non_preferred_falls_through() {
    let mtls = StubProvider::new("mtls", Err(transient_mtls_failure()));
    let pat = StubProvider::new("pat", Ok(ok_token()));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let token = evaluate_chain(&chain, None).expect("transient on non-preferred must fall through");
    assert_eq!(token.token.expose(), "stub");
    assert_eq!(pat.call_count(), 1);
}

#[test]
fn no_provider_succeeds_yields_none_succeeded() {
    let mtls = StubProvider::new("mtls", Err(AuthError::NotConfigured));
    let pat = StubProvider::new("pat", Err(AuthError::NotConfigured));

    let chain: Vec<&dyn AuthProvider> = vec![&mtls, &pat];
    let err = evaluate_chain(&chain, None).expect_err("nothing configured");
    assert!(matches!(err, ChainError::NoneSucceeded));
}

#[test]
fn is_terminal_classifies_gateway_variants() {
    assert!(AuthFailedSource::Gateway(GatewayError::PubkeyMissing).is_terminal());
    assert!(AuthFailedSource::Gateway(GatewayError::UnsafePath("..".into())).is_terminal());
    let transient = AuthFailedSource::Gateway(GatewayError::HealthCheck(Box::new(
        ureq::Error::Status(503, ureq::Response::new(503, "x", "x").unwrap()),
    )));
    assert!(!transient.is_terminal());
}
