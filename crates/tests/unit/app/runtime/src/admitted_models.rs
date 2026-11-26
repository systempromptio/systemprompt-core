//! The `admitted-models` conferral: a member that declared it reads a handle
//! only from the issuer `gateway-http` provided sealed. A member that
//! declared nothing, a host without the gateway member, and an issuer seeded
//! by anyone else (here the host, standing in for an install that forges one)
//! all read the typed missing dependency, never a handle and never an empty
//! list. The handle's surface is `AdmittedModels::admitted` alone, so no
//! authz hook or secret name is reachable through it.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::reserved::InstallConferral;
use systemprompt_extension::{Admitted, ConferralError, ProvideError, Resolved};
use systemprompt_identifiers::{JwtToken, ModelId};
use systemprompt_runtime::{AppContext, Kernel, KernelError};
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, probe_admitted, test_database_url,
};
use systemprompt_traits::{
    AdmittedModels, AdmittedModelsError, BackgroundTasks, ConferralIssuer, DynConferralIssuer,
    MemberGrant,
};

#[derive(Clone, Debug)]
struct Observed(Option<ConferralError>);

#[derive(Debug)]
struct ForgedIssuer;

#[derive(Debug)]
struct EveryModel;

#[async_trait]
impl AdmittedModels for EveryModel {
    async fn admitted(
        &self,
        _bearer: &JwtToken,
        candidates: Option<&[ModelId]>,
    ) -> Result<Vec<ModelId>, AdmittedModelsError> {
        Ok(candidates.map(<[ModelId]>::to_vec).unwrap_or_default())
    }
}

impl ConferralIssuer<dyn AdmittedModels> for ForgedIssuer {
    fn issue(&self, _grant: &MemberGrant) -> Arc<dyn AdmittedModels> {
        Arc::new(EveryModel)
    }
}

fn observing(id: &'static str, declare: bool) -> Probe {
    Probe::new(id, move |c| {
        if declare {
            c.conferral(InstallConferral::AdmittedModels);
        }
        c.provide(|r: &Resolved<'_>| -> Result<Observed, ProvideError> {
            Ok(Observed(r.admitted_models().err()))
        });
    })
}

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn resolve(ctx: &AppContext, probes: Vec<Probe>, forged: bool) -> Result<Kernel, KernelError> {
    let mut state = ctx.kernel().state().clone();
    if forged {
        let issuer: DynConferralIssuer<dyn AdmittedModels> = Arc::new(ForgedIssuer);
        state.insert_sealed(issuer);
    }
    let admitted: Admitted = probe_admitted(ComposedParts {
        probes,
        ..ComposedParts::default()
    });
    let mut kernel = Kernel::detached(BackgroundTasks::new(), state);
    kernel.resolve(&admitted)?;
    Ok(kernel)
}

fn missing(kernel: &Kernel) -> bool {
    let observed = kernel.state().get::<Observed>().expect("provided");
    matches!(observed.0, Some(ConferralError::Missing(_)))
}

#[tokio::test]
async fn a_member_that_declared_nothing_gets_no_handle() {
    let ctx = context().await;
    let kernel =
        resolve(&ctx, vec![observing("models-undeclared", false)], false).expect("resolves");
    assert!(missing(&kernel));
}

#[tokio::test]
async fn without_the_gateway_member_a_declared_read_is_missing_not_empty() {
    let ctx = context().await;
    let kernel =
        resolve(&ctx, vec![observing("models-no-gateway", true)], false).expect("resolves");
    assert!(missing(&kernel));
}

#[tokio::test]
async fn an_issuer_not_provided_by_gateway_http_is_never_handed_out() {
    let ctx = context().await;
    let kernel = resolve(&ctx, vec![observing("models-forged", true)], true).expect("resolves");
    assert!(
        missing(&kernel),
        "an issuer seeded by anyone but gateway-http must not answer for the instance"
    );
}

#[test]
fn the_row_names_gateway_http_as_its_only_provider() {
    assert_eq!(InstallConferral::AdmittedModels.name(), "admitted-models");
    assert_eq!(InstallConferral::AdmittedModels.provider(), "gateway-http");
}
