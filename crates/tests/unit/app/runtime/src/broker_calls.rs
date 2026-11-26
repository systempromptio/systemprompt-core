//! The broker-call verifier: a member that declared `BrokerCalls` reads a
//! handle that accepts core's credential-broker header and refuses a wrong
//! or absent one, holds no secret it can show, is bound to its member, and is
//! refused to a member that declared nothing; a child host withholds it.

use std::sync::Arc;

use axum::http::HeaderMap;
use systemprompt_extension::reserved::InstallConferral;
use systemprompt_extension::{
    Admitted, ConferralError, ConferralsWithheld, ProvideError, Resolved,
};
use systemprompt_identifiers::headers::CREDENTIAL_BROKER;
use systemprompt_runtime::{AdmitFn, AppContext, HostKernel, Kernel, KernelError};
use systemprompt_security::conferred::BrokerCallVerifiers;
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, host_child_env, host_probe_admitted,
    probe_admitted, seed_fixture_system_admin, test_database_url, test_db_pool,
};
use systemprompt_traits::{
    BackgroundTasks, BrokerCallRefused, BrokerCallVerifier, DynConferralIssuer,
};

const SECRET: &str = "broker-secret-under-test";

#[derive(Clone)]
struct Verifier(Arc<dyn BrokerCallVerifier>);

#[derive(Clone, Debug)]
struct Observed(Option<ConferralError>);

fn declaring(id: &'static str) -> Probe {
    Probe::new(id, |c| {
        c.conferral(InstallConferral::BrokerCalls);
        c.provide(|r: &Resolved<'_>| -> Result<Verifier, ProvideError> {
            Ok(Verifier(r.broker_calls()?))
        });
    })
}

fn undeclared() -> Probe {
    Probe::new("broker-undeclared", |c| {
        c.provide(|r: &Resolved<'_>| -> Result<Observed, ProvideError> {
            Ok(Observed(r.broker_calls().err()))
        });
    })
}

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn resolve(ctx: &AppContext, probes: Vec<Probe>) -> Result<Kernel, KernelError> {
    let mut state = ctx.kernel().state().clone();
    let issuer: DynConferralIssuer<dyn BrokerCallVerifier> =
        Arc::new(BrokerCallVerifiers::from_secret(SECRET).expect("a non-empty secret"));
    state.insert_sealed(issuer);
    let admitted: Admitted = probe_admitted(ComposedParts {
        probes,
        ..ComposedParts::default()
    });
    let mut kernel = Kernel::detached(BackgroundTasks::new(), state);
    kernel.resolve(&admitted)?;
    Ok(kernel)
}

fn headers(value: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(value) = value {
        headers.insert(CREDENTIAL_BROKER, value.parse().expect("a header value"));
    }
    headers
}

#[tokio::test]
async fn a_declared_verifier_accepts_the_broker_and_refuses_anything_else() {
    let ctx = context().await;
    let kernel = resolve(&ctx, vec![declaring("broker-member")]).expect("a declared row resolves");
    let Verifier(verifier) = kernel.state().get::<Verifier>().expect("provided");
    assert_eq!(verifier.verify(&headers(Some(SECRET))), Ok(()));
    assert_eq!(
        verifier.verify(&headers(Some("broker-secret-under-tesT"))),
        Err(BrokerCallRefused::Mismatch)
    );
    assert_eq!(
        verifier.verify(&headers(Some(""))),
        Err(BrokerCallRefused::Mismatch)
    );
    assert_eq!(
        verifier.verify(&headers(None)),
        Err(BrokerCallRefused::Absent)
    );
    let shown = format!("{verifier:?}");
    assert!(!shown.contains(SECRET), "{shown}");
    assert!(
        shown.contains("broker-member"),
        "the handle names its member: {shown}"
    );
    let issuer = BrokerCallVerifiers::from_secret(SECRET).expect("issuer");
    assert!(!format!("{issuer:?}").contains(SECRET));
    assert!(
        BrokerCallVerifiers::from_secret("").is_none(),
        "an empty secret seeds no issuer"
    );
}

#[tokio::test]
async fn a_member_that_declared_nothing_gets_no_verifier() {
    let ctx = context().await;
    let kernel = resolve(&ctx, vec![undeclared()]).expect("resolves");
    let observed = kernel.state().get::<Observed>().expect("provided");
    assert!(
        matches!(observed.0, Some(ConferralError::Missing(_))),
        "{observed:?}"
    );
}

#[tokio::test]
async fn a_child_host_withholds_the_verifier() {
    let bootstrap = ensure_test_bootstrap();
    seed_fixture_system_admin(&test_db_pool().await)
        .await
        .expect("seed the system admin");
    let env = host_child_env(bootstrap);
    let admit: AdmitFn =
        Box::new(|_profile| Ok(host_probe_admitted(vec![declaring("broker-child")])));
    let parts = HostKernel::boot(
        systemprompt_extension::artifact_ui::HostRole::McpServer,
        move |name| env.get(name).cloned(),
        admit,
    )
    .await
    .unwrap_or_else(|e| panic!("the child boots with the verifier withheld: {e:#}"));
    let state = parts.kernel_state();
    assert!(state.find::<ConferralsWithheld>().is_some());
    assert!(state.find::<Verifier>().is_none());
    assert!(
        state
            .find::<DynConferralIssuer<dyn BrokerCallVerifier>>()
            .is_none()
    );
}
