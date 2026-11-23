//! Member-declared subject dimensions for the RBAC resolver.
//!
//! Core's resolver knows two subject dimensions, `user` and `role`, plus the
//! dimensions its own providers declare (`traffic`). Anything else an operator
//! wants to write rules against — department, cost centre, clearance,
//! jurisdiction — is a tenant concept, and core deliberately does not learn
//! it. What core provides instead is the mechanism:
//!
//! 1. A member mints a [`RuleType`] with [`RuleType::extension`] and describes
//!    it as a [`SubjectDimension`], choosing where it slots in the precedence
//!    ladder.
//! 2. It implements [`SubjectAttributeProvider`] to look up that dimension's
//!    values for a user, and contributes the provider with
//!    `Contributions::subject_attributes`.
//! 3. [`SubjectProviderSet::from_admitted`] runs every admitted member's
//!    factory once — core members first, then installs in composition order —
//!    and refuses a set in which two providers declare one dimension
//!    ([`SubjectProviderError::DimensionConflict`]) or an install declares a
//!    reserved core dimension ([`SubjectProviderError::ReservedDimension`]),
//!    `role` included: only a core member provides roles. No provider may
//!    declare `user`, which the resolver reads off the request itself.
//! 4. The enforcement path calls [`gather_subject_attributes`] once per request
//!    and hands the resulting [`SubjectAttributes`] to
//!    [`resolve`][super::resolver::resolve] alongside the dimension list.
//!
//! Values are resolved by lookup rather than read from JWT claims, so a
//! department change or a revocation takes effect on the next request instead
//! of lingering until the token refreshes.
//!
//! The split between metadata ([`SubjectDimension`], no behaviour) and
//! behaviour ([`SubjectAttributeProvider`], async I/O) is what keeps
//! [`resolve`][super::resolver::resolve] pure and synchronous: gathering is
//! the only async step, and it happens before the resolver runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

use systemprompt_extension::{BuiltState, FactoryError, Resolved, StandingTag};
use systemprompt_identifiers::ExtensionId;
pub use systemprompt_models::authz::{
    SubjectAttributeSource, SubjectAttributes, SubjectSourceError,
};
pub use systemprompt_traits::authz::{
    RESERVED_SUBJECT_DIMENSIONS, ROLE_PRECEDENCE, SubjectAttributeError, SubjectAttributeProvider,
    SubjectDimension, USER_PRECEDENCE,
};

use super::types::RuleType;

pub static NO_SUBJECT_ATTRIBUTES: SubjectAttributes = SubjectAttributes::EMPTY;

/// Shared handle to a contributed provider.
pub type SharedSubjectAttributeProvider = Arc<dyn SubjectAttributeProvider>;

/// Why the subject provider set could not be built.
#[derive(Debug, thiserror::Error)]
pub enum SubjectProviderError {
    #[error("subject provider factory of {owner} failed")]
    Factory {
        owner: ExtensionId,
        #[source]
        source: FactoryError,
    },
    #[error("subject dimension {dimension} declared by {second}, already owned by {first}")]
    DimensionConflict {
        dimension: RuleType,
        first: ExtensionId,
        second: ExtensionId,
    },
    #[error("{owner} declares reserved subject dimension {dimension}")]
    ReservedDimension {
        owner: ExtensionId,
        dimension: RuleType,
    },
}

/// One contributed provider with the member that contributed it.
#[derive(Debug, Clone)]
pub struct SubjectProviderEntry {
    pub owner: ExtensionId,
    pub standing: StandingTag,
    pub provider: SharedSubjectAttributeProvider,
}

#[must_use]
pub fn dimensions_of(providers: &[SharedSubjectAttributeProvider]) -> Vec<SubjectDimension> {
    providers.iter().map(|p| p.dimension()).collect()
}

#[must_use]
pub fn default_attribution_value(
    provider: &SharedSubjectAttributeProvider,
    held: &[String],
) -> Option<String> {
    if provider.default_attribution() {
        held.first().cloned()
    } else {
        None
    }
}

pub async fn gather_subject_attributes(
    providers: &[SharedSubjectAttributeProvider],
    user_id: &UserId,
) -> Result<SubjectAttributes, super::AuthzError> {
    let mut attributes = SubjectAttributes::new();
    for provider in providers {
        attributes.insert(
            provider.dimension().rule_type,
            provider.values_for(user_id).await?,
        );
    }
    Ok(attributes)
}

fn refuse_reserved(entry: &SubjectProviderEntry) -> Result<(), SubjectProviderError> {
    let dimension = entry.provider.dimension().rule_type;
    let request_dimension = dimension == RuleType::USER;
    let reserved = RESERVED_SUBJECT_DIMENSIONS
        .iter()
        .any(|d| d.rule_type == dimension);
    if request_dimension || (reserved && entry.standing == StandingTag::Install) {
        return Err(SubjectProviderError::ReservedDimension {
            owner: entry.owner.clone(),
            dimension,
        });
    }
    Ok(())
}

/// The admitted subject-attribute providers, built once from the admitted
/// composition and shared by every consumer that needs to look a dimension up
/// by name (gateway scope attribution, quota subjects).
#[derive(Debug, Clone, Default)]
pub struct SubjectProviderSet {
    providers: Arc<[SharedSubjectAttributeProvider]>,
    dimensions: Arc<[SubjectDimension]>,
}

impl SubjectProviderSet {
    pub fn from_admitted(
        built: &BuiltState,
        r: &Resolved<'_>,
    ) -> Result<Self, SubjectProviderError> {
        let mut entries = Vec::new();
        for member in built.members() {
            for factory in member.sink.subject_providers() {
                let provider = factory
                    .build(&r.scoped_to(&member.owner, member.standing))
                    .map_err(|source| SubjectProviderError::Factory {
                        owner: member.owner.clone(),
                        source,
                    })?;
                entries.push(SubjectProviderEntry {
                    owner: member.owner.clone(),
                    standing: member.standing,
                    provider,
                });
            }
        }
        Self::new(entries)
    }

    pub fn new(mut entries: Vec<SubjectProviderEntry>) -> Result<Self, SubjectProviderError> {
        entries.sort_by_key(|entry| match entry.standing {
            StandingTag::Core => 0u8,
            StandingTag::Install => 1,
        });
        for (index, entry) in entries.iter().enumerate() {
            refuse_reserved(entry)?;
            let dimension = entry.provider.dimension().rule_type;
            if let Some(first) = entries[..index]
                .iter()
                .find(|earlier| earlier.provider.dimension().rule_type == dimension)
            {
                return Err(SubjectProviderError::DimensionConflict {
                    dimension,
                    first: first.owner.clone(),
                    second: entry.owner.clone(),
                });
            }
        }
        let providers: Vec<SharedSubjectAttributeProvider> =
            entries.into_iter().map(|entry| entry.provider).collect();
        Ok(Self {
            dimensions: dimensions_of(&providers).into(),
            providers: providers.into(),
        })
    }

    #[must_use]
    pub fn find(&self, dimension: &str) -> Option<&SharedSubjectAttributeProvider> {
        self.providers
            .iter()
            .find(|provider| provider.dimension().rule_type.as_str() == dimension)
    }

    #[must_use]
    pub fn registered_dimensions(&self) -> Vec<RuleType> {
        self.providers
            .iter()
            .map(|p| p.dimension().rule_type)
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &SharedSubjectAttributeProvider> {
        self.providers.iter()
    }

    pub async fn default_value(
        &self,
        user_id: &UserId,
        dimension: &str,
    ) -> Result<Option<String>, super::AuthzError> {
        let Some(provider) = self.find(dimension) else {
            return Ok(None);
        };
        if !provider.default_attribution() {
            return Ok(None);
        }
        let held = provider.values_for(user_id).await?;
        Ok(default_attribution_value(provider, &held))
    }

    pub async fn gather(&self, user_id: &UserId) -> Result<SubjectAttributes, super::AuthzError> {
        gather_subject_attributes(&self.providers, user_id).await
    }
}

fn source_error(dimension: &RuleType, source: SubjectAttributeError) -> SubjectSourceError {
    SubjectSourceError::Provider {
        dimension: dimension.as_str().to_owned(),
        source: source.into(),
    }
}

#[async_trait]
impl SubjectAttributeSource for SubjectProviderSet {
    fn dimensions(&self) -> &[SubjectDimension] {
        &self.dimensions
    }

    async fn values_for(
        &self,
        user: &UserId,
        dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError> {
        let provider = self.find(dimension.rule_type.as_str()).ok_or_else(|| {
            SubjectSourceError::UnknownDimension(dimension.rule_type.as_str().to_owned())
        })?;
        provider
            .values_for(user)
            .await
            .map_err(|e| source_error(&dimension.rule_type, e))
    }

    async fn resolve_all(&self, user: &UserId) -> Result<SubjectAttributes, SubjectSourceError> {
        let mut attributes = SubjectAttributes::new();
        for provider in self.providers.iter() {
            let rule_type = provider.dimension().rule_type;
            let values = provider
                .values_for(user)
                .await
                .map_err(|e| source_error(&rule_type, e))?;
            attributes.insert(rule_type, values);
        }
        Ok(attributes)
    }

    async fn known_values(
        &self,
        dimension: &SubjectDimension,
    ) -> Result<Option<Vec<String>>, SubjectSourceError> {
        let provider = self.find(dimension.rule_type.as_str()).ok_or_else(|| {
            SubjectSourceError::UnknownDimension(dimension.rule_type.as_str().to_owned())
        })?;
        provider
            .known_values()
            .await
            .map_err(|e| source_error(&dimension.rule_type, e))
    }
}
