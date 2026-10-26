//! Publication gates: extension-registered checks a publication must pass
//! before its transaction opens.
//!
//! An extension implements [`PublicationGate`] and registers a factory with
//! [`register_publication_gate!`][crate::register_publication_gate]. The
//! composition root instantiates every registration once, ordered by
//! descending priority, into a [`PublicationPolicy`]. Gating applies to
//! `PublishImprovement`, and to `InitialAdoption` when
//! `marketplace.publication.gate.on_adoption` is set. Every gate must admit;
//! a refusal or a gate failure refuses the publication, and
//! `marketplace.publication.gate.required` with no registered gate refuses
//! every gated publication.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::authz::SubjectSourceHandle;

use crate::registry::MarketplaceFactoryContext;
use systemprompt_governance::authz::{
    AuthzHookContext, NullAuditSink, SharedRoleDirectory, SubjectProviderSet,
    discover_role_directory,
};
use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, UserId};
use systemprompt_manifest::services::ManagedMarketplaceConfig;
use systemprompt_traits::BoxedSource;

use super::{
    AssetDigest, CohortSelector, ComparisonEvidence, ManagedError, PublicationAction, ResourceKind,
    Result,
};

/// A check a managed publication must pass before it is recorded.
///
/// The returned evidence digest is stored with the review, so it must be the
/// digest of evidence the gate can reproduce or retain. `#[async_trait]`
/// because gates are held as `Arc<dyn PublicationGate>`.
#[async_trait]
pub trait PublicationGate: Send + Sync {
    fn id(&self) -> &'static str;

    async fn admit(
        &self,
        ctx: &GateContext<'_>,
    ) -> std::result::Result<GateVerdict, PublicationGateError>;
}

#[derive(Debug, Clone, Copy)]
pub struct GateContext<'a> {
    pub owner: &'a UserId,
    pub reviewer: &'a UserId,
    pub action: PublicationAction,
    pub resource_id: &'a ManagedResourceId,
    pub resource_kind: ResourceKind,
    pub revision_id: Option<&'a ResourceRevisionId>,
    pub bundle_digest: Option<&'a AssetDigest>,
    pub cohort: Option<&'a CohortSelector>,
    pub comparison_evidence: &'a ComparisonEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    Admit { evidence_digest: AssetDigest },
    Refuse { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum PublicationGateError {
    #[error("publication gate backend unavailable: {0}")]
    Backend(#[source] BoxedSource),
    #[error("publication gate evidence is invalid: {0}")]
    Evidence(String),
}

type PublicationGateFactory =
    fn(
        &MarketplaceFactoryContext,
    ) -> std::result::Result<Arc<dyn PublicationGate>, PublicationGateError>;

#[derive(Debug, Clone, Copy)]
pub struct PublicationGateRegistration {
    pub factory: PublicationGateFactory,
    pub priority: i32,
}

inventory::collect!(PublicationGateRegistration);

#[must_use]
pub fn discover_publication_gates() -> Vec<&'static PublicationGateRegistration> {
    let mut all: Vec<&PublicationGateRegistration> =
        inventory::iter::<PublicationGateRegistration>().collect();
    all.sort_by_key(|reg| std::cmp::Reverse(reg.priority));
    all
}

#[macro_export]
macro_rules! register_publication_gate {
    ($factory:expr, priority = $priority:expr $(,)?) => {
        ::inventory::submit! {
            $crate::managed::PublicationGateRegistration {
                factory: $factory,
                priority: $priority,
            }
        }
    };
    ($factory:expr $(,)?) => {
        $crate::register_publication_gate!($factory, priority = 0);
    };
}

/// The publication policy a [`ManagedRepository`][super::ManagedRepository]
/// enforces.
///
/// It holds the services `marketplace:` block, the instantiated gates in
/// priority order, and the subject providers cohort membership is read from.
#[derive(Clone, Default)]
pub struct PublicationPolicy {
    pub(crate) config: ManagedMarketplaceConfig,
    gates: Vec<Arc<dyn PublicationGate>>,
    pub(crate) subjects: SubjectProviderSet,
    pub(crate) roles: Option<SharedRoleDirectory>,
}

impl std::fmt::Debug for PublicationPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicationPolicy")
            .field("config", &self.config)
            .field(
                "gates",
                &self.gates.iter().map(|g| g.id()).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GateAdmission {
    pub gate_id: String,
    pub evidence_digest: AssetDigest,
}

impl PublicationPolicy {
    #[must_use]
    pub fn new(
        config: ManagedMarketplaceConfig,
        gates: Vec<Arc<dyn PublicationGate>>,
        subjects: SubjectProviderSet,
        roles: Option<SharedRoleDirectory>,
    ) -> Self {
        Self {
            config,
            gates,
            subjects,
            roles,
        }
    }

    pub fn discover(
        db: &DbPool,
        config: ManagedMarketplaceConfig,
        services: ServicesView,
        subjects: SubjectSourceHandle,
        providers: SubjectProviderSet,
    ) -> Result<Self> {
        let factory_ctx = MarketplaceFactoryContext {
            db: Arc::clone(db),
            services: services.clone(),
            subjects: subjects.clone(),
        };
        let gates = discover_publication_gates()
            .into_iter()
            .map(|registration| (registration.factory)(&factory_ctx))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|source| ManagedError::Internal {
                context: "publication gate factory",
                source: source.into(),
            })?;
        let hook_ctx = AuthzHookContext {
            pool: db.pool(),
            sink: Arc::new(NullAuditSink),
            services,
            subjects,
        };
        Ok(Self::new(
            config,
            gates,
            providers,
            discover_role_directory(&hook_ctx),
        ))
    }

    #[must_use]
    pub const fn config(&self) -> &ManagedMarketplaceConfig {
        &self.config
    }

    const fn gates_action(&self, action: PublicationAction) -> bool {
        match action {
            PublicationAction::PublishImprovement => true,
            PublicationAction::InitialAdoption => self.config.publication.gate.on_adoption,
            PublicationAction::Withdraw | PublicationAction::Rollback => false,
        }
    }

    pub(super) async fn admit(&self, ctx: &GateContext<'_>) -> Result<Option<GateAdmission>> {
        if !self.gates_action(ctx.action) {
            return Ok(None);
        }
        if self.gates.is_empty() {
            if self.config.publication.gate.required {
                tracing::warn!(
                    resource_id = %ctx.resource_id,
                    "publication refused: a publication gate is required and none is registered"
                );
                return Err(ManagedError::Unavailable);
            }
            return Ok(None);
        }
        let mut admitted = Vec::with_capacity(self.gates.len());
        for gate in &self.gates {
            match gate.admit(ctx).await {
                Ok(GateVerdict::Admit { evidence_digest }) => {
                    admitted.push((gate.id(), evidence_digest));
                },
                Ok(GateVerdict::Refuse { reason }) => {
                    return Err(ManagedError::Conflict(format!(
                        "Publication gate {} refused the publication: {reason}",
                        gate.id()
                    )));
                },
                Err(source) => {
                    return Err(ManagedError::Internal {
                        context: "publication gate",
                        source: source.into(),
                    });
                },
            }
        }
        Ok(Some(combine(admitted)?))
    }
}

fn combine(mut admitted: Vec<(&'static str, AssetDigest)>) -> Result<GateAdmission> {
    if admitted.len() == 1 {
        let (gate_id, evidence_digest) = admitted.remove(0);
        return Ok(GateAdmission {
            gate_id: gate_id.to_owned(),
            evidence_digest,
        });
    }
    let gate_id = admitted
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>()
        .join(",");
    let evidence_digest = AssetDigest::of(&serde_jcs::to_vec(&admitted)?);
    Ok(GateAdmission {
        gate_id,
        evidence_digest,
    })
}
