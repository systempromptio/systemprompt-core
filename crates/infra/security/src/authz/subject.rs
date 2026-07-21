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

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

use super::registry::AuthzHookContext;
use super::types::RuleType;

/// Precedence of core's `user` dimension. Nothing may bind tighter.
pub const USER_PRECEDENCE: u16 = 0;
/// Precedence of core's `role` dimension. Extensions slot below this value to
/// outrank roles, above it to yield to them.
pub const ROLE_PRECEDENCE: u16 = 200;

/// Describes one subject dimension to the resolver.
///
/// Metadata only: no behaviour and no I/O, so it is cheap to clone and safe to
/// hold in a `const`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectDimension {
    /// The `access_control_rules.rule_type` slug this dimension owns.
    pub rule_type: RuleType,
    /// Operator-facing label, e.g. for the access matrix column header.
    pub label: &'static str,
    /// Lower binds tighter. Core uses [`USER_PRECEDENCE`] and
    /// [`ROLE_PRECEDENCE`]; an extension dimension at 100 outranks a role rule
    /// and yields to a user rule.
    pub precedence: u16,
}

/// Subject values per dimension, gathered before the pure resolver runs.
///
/// A dimension with no values for the user is simply absent, which makes its
/// rules unmatchable for that request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubjectAttributes(BTreeMap<RuleType, Vec<String>>);

impl SubjectAttributes {
    /// No values for any dimension. Const so callers with no extension
    /// dimensions registered can pass `&SubjectAttributes::EMPTY` without a
    /// binding.
    pub const EMPTY: Self = Self(BTreeMap::new());

    #[must_use]
    pub const fn new() -> Self {
        Self::EMPTY
    }

    /// Records the values for one dimension, replacing any previous entry.
    pub fn insert(&mut self, rule_type: RuleType, values: Vec<String>) {
        self.0.insert(rule_type, values);
    }

    /// Values the user holds for `rule_type`, empty when the dimension is
    /// unregistered or the user has no value for it.
    #[must_use]
    pub fn values(&self, rule_type: &RuleType) -> &[String] {
        self.0.get(rule_type).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(RuleType, Vec<String>)> for SubjectAttributes {
    fn from_iter<I: IntoIterator<Item = (RuleType, Vec<String>)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Shared `'static` empty attribute set.
///
/// [`SubjectAttributes::EMPTY`] cannot be const-promoted behind a reference
/// (its map owns a heap allocation in the general case), so call sites that
/// need a borrow outliving the expression — a helper returning a
/// [`ResolveInput`][super::resolver::ResolveInput], a resolver call with no
/// dimensions registered — borrow this instead.
pub static NO_SUBJECT_ATTRIBUTES: SubjectAttributes = SubjectAttributes::EMPTY;

/// Looks up the values a user holds for one extension-owned dimension.
///
/// `#[async_trait]` because providers are held as
/// [`SharedSubjectAttributeProvider`], an `Arc<dyn …>`, so the trait must stay
/// `dyn`-compatible.
#[async_trait]
pub trait SubjectAttributeProvider: Send + Sync + Debug {
    /// The dimension this provider supplies. Must be stable for the process
    /// lifetime; the resolver builds its precedence ladder from it.
    fn dimension(&self) -> SubjectDimension;

    /// Values for `user_id`, or empty when the user has none. Implementations
    /// should fail soft: a lookup error is an absent attribute, not a deny,
    /// because the resolver's own deny paths already close the default.
    async fn values_for(&self, user_id: &UserId) -> Vec<String>;
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

/// The dimension list to hand [`resolve`][super::resolver::resolve], derived
/// from the providers gathered for the same request.
#[must_use]
pub fn dimensions_of(providers: &[SharedSubjectAttributeProvider]) -> Vec<SubjectDimension> {
    providers.iter().map(|p| p.dimension()).collect()
}

/// Gathers every provider's values for `user_id`.
///
/// The only async step in the authorization path and the only place a provider
/// is called; everything downstream operates on the returned snapshot.
pub async fn gather_subject_attributes(
    providers: &[SharedSubjectAttributeProvider],
    user_id: &UserId,
) -> SubjectAttributes {
    let mut attributes = SubjectAttributes::new();
    for provider in providers {
        let dimension = provider.dimension();
        let values = provider.values_for(user_id).await;
        attributes.insert(dimension.rule_type, values);
    }
    attributes
}

/// Register an extension subject-attribute provider at static-init time.
///
/// The factory receives a borrowed [`AuthzHookContext`] (pool + audit sink)
/// and returns the constructed provider. Wire alongside `register_extension!`
/// in the extension's `extension.rs`:
///
/// ```ignore
/// systemprompt_security::register_subject_attribute_provider!(|ctx| {
///     std::sync::Arc::new(DepartmentAttributeProvider::new(ctx.pool.clone()))
///         as systemprompt_security::authz::SharedSubjectAttributeProvider
/// });
/// ```
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
