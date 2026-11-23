//! Every factory kind a member's sink stores runs only through a scoped
//! resolution: an install member registering a factory of each
//! `FactoryKind`, each asking for the signing keys, fails admission (the
//! kinds the kernel builds at resolve) or the runtime build (the kinds the
//! API builds: runner services, readiness checks, diagnostic probes) typed.
//! The kinds are walked from `FactoryKind::ALL`, and the registration below
//! matches every variant, so a new kind fails to compile here until it is
//! covered.

use std::sync::Arc;

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::notifications::NotificationAudienceResolver;
use systemprompt_extension::{
    DiagnosticProbe, FactoryError, FactoryKind, GatewayRequestGuard, ReadinessCheck, Resolved,
};
use systemprompt_provider_contracts::governance::GovernancePolicy;
use systemprompt_runtime::Kernel;
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, probe_admitted, test_database_url,
};
use systemprompt_traits::authz::SubjectAttributeProvider;
use systemprompt_traits::{BackgroundTasks, RunnerService, SecretReferences};

fn reached() -> FactoryError {
    FactoryError::Failed(Box::new(std::io::Error::other(
        "the signing keys were served to an install factory",
    )))
}

macro_rules! asks_for_signing_keys {
    ($t:ty) => {
        |r: &Resolved<'_>| -> Result<Arc<$t>, FactoryError> {
            r.get::<SigningKeys>()?;
            Err(reached())
        }
    };
}

fn register(kind: FactoryKind) -> Probe {
    Probe::new("forged-factories", move |c| match kind {
        FactoryKind::RunnerService => {
            c.runner_service(asks_for_signing_keys!(dyn RunnerService));
        },
        FactoryKind::Readiness => {
            c.readiness(asks_for_signing_keys!(dyn ReadinessCheck));
        },
        FactoryKind::Diagnostic => {
            c.diagnostic(asks_for_signing_keys!(dyn DiagnosticProbe));
        },
        FactoryKind::GatewayGuard => {
            c.gateway_guard(asks_for_signing_keys!(dyn GatewayRequestGuard));
        },
        FactoryKind::SubjectProvider => {
            c.subject_attributes(asks_for_signing_keys!(dyn SubjectAttributeProvider));
        },
        FactoryKind::Policy => {
            c.policy(asks_for_signing_keys!(dyn GovernancePolicy));
        },
        FactoryKind::AudienceResolver => {
            c.audience_resolver(asks_for_signing_keys!(dyn NotificationAudienceResolver));
        },
        FactoryKind::SecretReferences => {
            c.secret_references(asks_for_signing_keys!(dyn SecretReferences));
        },
    })
}

fn names_the_signing_keys(error: &dyn std::fmt::Debug) -> bool {
    format!("{error:?}").contains(std::any::type_name::<SigningKeys>())
}

#[tokio::test]
async fn an_install_factory_of_every_kind_is_refused_the_signing_keys_typed() {
    ensure_test_bootstrap();
    let ctx = systemprompt_test_fixtures::test_app_context(&test_database_url()).await;
    for kind in FactoryKind::ALL {
        let admitted = probe_admitted(ComposedParts {
            probes: vec![register(*kind)],
            ..ComposedParts::default()
        });
        let member = admitted
            .built()
            .members()
            .find(|m| m.owner.as_str() == "forged-factories")
            .expect("the probe is admitted");
        for other in FactoryKind::ALL {
            assert_eq!(
                member.sink.factory_count(*other),
                usize::from(other == kind),
                "{kind:?} registers exactly one factory of its own kind"
            );
        }
        let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
        let refusal = match kernel.resolve(&admitted) {
            Err(error) => format!("{error:?}"),
            Ok(()) => {
                let built = admitted.built();
                let error = match kind {
                    FactoryKind::RunnerService => kernel
                        .admitted_factories(
                            built,
                            "runner_service",
                            ContributionSink::runner_services,
                        )
                        .map(|_| ()),
                    FactoryKind::Readiness => kernel
                        .admitted_factories(built, "readiness", ContributionSink::readiness)
                        .map(|_| ()),
                    FactoryKind::Diagnostic => kernel
                        .admitted_factories(built, "diagnostic", ContributionSink::diagnostics)
                        .map(|_| ()),
                    FactoryKind::GatewayGuard
                    | FactoryKind::SubjectProvider
                    | FactoryKind::Policy
                    | FactoryKind::AudienceResolver
                    | FactoryKind::SecretReferences => {
                        panic!("{kind:?} is built at resolve and must refuse there")
                    },
                }
                .expect_err("the runtime build refuses the install's factory");
                format!("{error:?}")
            },
        };
        assert!(names_the_signing_keys(&refusal), "{kind:?}: {refusal}");
    }
}
