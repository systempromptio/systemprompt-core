//! Runtime resolution of a skill through an installation's managed-resource
//! authority.
//!
//! A skill key is either not managed at all (the caller may fall back to the
//! disk catalogue), published (its retained content is returned), or managed
//! but withheld — never adopted, withdrawn — in which case nothing is served
//! for that key and the disk copy must not be used either. Corrupt retained
//! content is an error, not a withholding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::{SkillId, UserId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedManagedSkill {
    pub id: SkillId,
    pub name: String,
    pub description: String,
    pub instructions: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithheldReason {
    NeverAdopted,
    NotGranted,
    Withdrawn,
}

impl WithheldReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotGranted => "not granted",
            Self::NeverAdopted => "never adopted",
            Self::Withdrawn => "withdrawn",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillResolution {
    NotManaged,
    Published(ResolvedManagedSkill),
    Withheld(WithheldReason),
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedSkillResolverError {
    #[error("managed skill `{key}` failed integrity verification")]
    Integrity { key: String },
    #[error("managed skill resolver unavailable: {0}")]
    Unavailable(String),
}

/// Held as `Arc<dyn ManagedSkillResolver>` so the agent runtime can use
/// whichever authority the composition root wires in without depending on
/// the marketplace domain; hence `#[async_trait]`.
#[async_trait]
pub trait ManagedSkillResolver: Send + Sync + std::fmt::Debug {
    async fn resolve_skill(
        &self,
        owner: &UserId,
        key: &str,
    ) -> Result<SkillResolution, ManagedSkillResolverError>;
}

pub type DynManagedSkillResolver = Arc<dyn ManagedSkillResolver>;
