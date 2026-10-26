//! Extension-declared subject dimensions for the RBAC resolver.
//!
//! Core's resolver knows two subject dimensions, `user` and `role`. Anything
//! else an operator wants to write rules against — department, cost centre,
//! clearance, jurisdiction — is a tenant concept, and core deliberately does
//! not learn it. What core provides instead is the mechanism:
//!
//! 1. An extension mints a [`RuleType`] with [`RuleType::extension`] and
//!    describes it as a [`SubjectDimension`], choosing where it slots in the
//!    precedence ladder.
//! 2. It implements [`SubjectAttributeProvider`] to look up that dimension's
//!    values for a user, and registers the provider with
//!    [`register_subject_attribute_provider!
//!    `][crate::register_subject_attribute_provider].
//! 3. The enforcement path calls [`gather_subject_attributes`] once per request
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

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

use super::registry::AuthzHookContext;
use super::types::RuleType;
pub use systemprompt_models::authz::{
    SubjectAttributeSource, SubjectAttributes, SubjectDimension, SubjectSourceError,
};

pub const USER_PRECEDENCE: u16 = 0;
pub const ROLE_PRECEDENCE: u16 = 200;

pub static NO_SUBJECT_ATTRIBUTES: SubjectAttributes = SubjectAttributes::EMPTY;

/// Looks up the values a user holds for one extension-owned dimension.
///
/// `default_attribution` says whether a request with no header and no key
/// binding for this dimension is attributed to the first value the user holds.
/// `false` makes the dimension header-only: such a request carries no
/// attribution for it, while a header or key binding is still verified against
/// `values_for`.
///
/// `key_bound_values` are values no user holds: an API key bound to one
/// carries it, and a header may name it only when the authenticating key is
/// bound to that same value. Key issuance accepts them without a membership
/// check, so binding one is the issuer's authority, never the caller's.
///
/// `#[async_trait]` because providers are held as
/// [`SharedSubjectAttributeProvider`], an `Arc<dyn …>`, so the trait must stay
/// `dyn`-compatible.
#[async_trait]
pub trait SubjectAttributeProvider: Send + Sync + Debug {
    fn dimension(&self) -> SubjectDimension;

    async fn values_for(&self, user_id: &UserId) -> Result<Vec<String>, super::AuthzError>;

    fn default_attribution(&self) -> bool {
        true
    }

    fn key_bound_values(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Shared handle to a registered provider.
pub type SharedSubjectAttributeProvider = Arc<dyn SubjectAttributeProvider>;

/// One inventory submission per
/// [`register_subject_attribute_provider!
/// `][crate::register_subject_attribute_provider] call. The factory runs once
/// at `AppContext` build time and must not block.
#[derive(Debug, Clone, Copy)]
pub struct SubjectProviderRegistration {
    pub factory: fn(&AuthzHookContext) -> SharedSubjectAttributeProvider,
}

inventory::collect!(SubjectProviderRegistration);

#[must_use]
pub fn discover_subject_providers(ctx: &AuthzHookContext) -> Vec<SharedSubjectAttributeProvider> {
    inventory::iter::<SubjectProviderRegistration>()
        .map(|reg| (reg.factory)(ctx))
        .collect()
}

#[must_use]
pub fn dimensions_of(providers: &[SharedSubjectAttributeProvider]) -> Vec<SubjectDimension> {
    providers.iter().map(|p| p.dimension()).collect()
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

#[macro_export]
macro_rules! register_subject_attribute_provider {
    ($factory:expr) => {
        ::inventory::submit! {
            $crate::authz::SubjectProviderRegistration {
                factory: $factory,
            }
        }
    };
}

/// The registered subject-attribute providers, discovered once at a
/// composition root and shared by every consumer that needs to look a
/// dimension up by name (gateway scope attribution, quota subjects).
#[derive(Debug, Clone, Default)]
pub struct SubjectProviderSet {
    providers: Arc<[SharedSubjectAttributeProvider]>,
    dimensions: Arc<[SubjectDimension]>,
}

impl SubjectProviderSet {
    #[must_use]
    pub fn discover(ctx: &AuthzHookContext) -> Self {
        Self::from_providers(discover_subject_providers(ctx))
    }

    #[must_use]
    pub fn from_providers(providers: Vec<SharedSubjectAttributeProvider>) -> Self {
        Self {
            dimensions: dimensions_of(&providers).into(),
            providers: providers.into(),
        }
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

    pub async fn gather(&self, user_id: &UserId) -> Result<SubjectAttributes, super::AuthzError> {
        gather_subject_attributes(&self.providers, user_id).await
    }
}

fn source_error(dimension: &RuleType, source: super::AuthzError) -> SubjectSourceError {
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
}
