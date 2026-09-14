//! Runtime contracts for resolving managed resources across domain boundaries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedManagedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
}

#[derive(Debug, thiserror::Error)]
#[error("managed skill resolution failed: {message}")]
pub struct ManagedSkillResolverError {
    pub message: String,
}

#[async_trait]
pub trait ManagedSkillResolver: Send + Sync + std::fmt::Debug {
    async fn resolve_skill(
        &self,
        owner: &UserId,
        key: &str,
    ) -> Result<Option<ResolvedManagedSkill>, ManagedSkillResolverError>;
}

pub type DynManagedSkillResolver = Arc<dyn ManagedSkillResolver>;
