//! Publication gates: contributed checks a publication must pass before its
//! transaction opens.
//!
//! An extension implements [`PublicationGate`] and contributes a factory to
//! [`MarketplaceSlot`][crate::contract::MarketplaceSlot]. The slot builds every
//! gate once, core members first, and the composition root assembles them
//! into a [`PublicationPolicy`] with [`PublicationPolicy::from_seams`]. Gating
//! applies to
//! `PublishImprovement`, and to `InitialAdoption` when
//! `marketplace.publication.gate.on_adoption` is set. Every gate must admit;
//! a refusal or a gate failure refuses the publication, and
//! `marketplace.publication.gate.required` with no contributed gate refuses
//! every gated publication. Because every gate must admit, the outcome does
//! not depend on gate order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;

use crate::contract::MarketplaceSeams;
use systemprompt_governance::authz::{SharedRoleDirectory, SubjectProviderSet};
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
    #[error("publication gate evidence is invalid: {message}")]
    Evidence { message: String },
}

/// The publication policy a [`ManagedRepository`][super::ManagedRepository]
/// enforces.
///
/// It holds the services `marketplace:` block, the built gates core first, and
/// the subject providers cohort membership is read from.
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

    #[must_use]
    pub fn from_seams(
        config: ManagedMarketplaceConfig,
        seams: &MarketplaceSeams,
        subjects: SubjectProviderSet,
    ) -> Self {
        Self::new(config, seams.gates().to_vec(), subjects, seams.roles())
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
                    "publication refused: a publication gate is required and none is contributed"
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
                    return Err(ManagedError::GateRefused {
                        gate: gate.id(),
                        reason,
                    });
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
