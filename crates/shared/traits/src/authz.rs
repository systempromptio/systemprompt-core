//! Subject-attribute providers and the open subject-dimension vocabulary.
//!
//! Core's resolver knows two subject dimensions, `user` and `role`, plus the
//! dimensions its own providers declare ([`RESERVED_SUBJECT_DIMENSIONS`]).
//! Anything else an operator writes rules against — department, cost centre,
//! clearance — is a member concept: the member mints a [`RuleType`] with
//! [`RuleType::extension`], describes it as a [`SubjectDimension`] and
//! contributes a [`SubjectAttributeProvider`] that looks up its values for a
//! user. A dimension is owned by exactly one provider; a member declaring a
//! reserved dimension, or two declaring one dimension, is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_provider_contracts::governance::RuleType;

use crate::repository::BoxedSource;

/// Describes one subject dimension to the resolver.
///
/// Metadata only: no behaviour and no I/O, so it is cheap to clone and safe to
/// hold in a `const`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectDimension {
    pub rule_type: RuleType,
    pub label: &'static str,
    pub precedence: u16,
}

pub const USER_PRECEDENCE: u16 = 0;
pub const ROLE_PRECEDENCE: u16 = 200;
pub const TRAFFIC_PRECEDENCE: u16 = 400;

pub const RESERVED_SUBJECT_DIMENSIONS: &[SubjectDimension] = &[
    SubjectDimension {
        rule_type: RuleType::USER,
        label: "User",
        precedence: USER_PRECEDENCE,
    },
    SubjectDimension {
        rule_type: RuleType::ROLE,
        label: "Role",
        precedence: ROLE_PRECEDENCE,
    },
    SubjectDimension {
        rule_type: RuleType::extension_static("traffic"),
        label: "Traffic",
        precedence: TRAFFIC_PRECEDENCE,
    },
];

#[derive(Debug, thiserror::Error)]
pub enum SubjectAttributeError {
    #[error("subject lookup failed")]
    Lookup(#[source] BoxedSource),
}

/// Looks up the values a user holds for one subject dimension.
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
/// `known_values` lists every value the dimension can take when the provider
/// can enumerate them; the default `None` means the values cannot be listed.
///
/// `#[async_trait]` because providers are held as `Arc<dyn …>`, so the trait
/// must stay `dyn`-compatible.
#[async_trait]
pub trait SubjectAttributeProvider: Send + Sync + Debug {
    fn dimension(&self) -> SubjectDimension;

    async fn values_for(&self, user_id: &UserId) -> Result<Vec<String>, SubjectAttributeError>;

    fn default_attribution(&self) -> bool {
        true
    }

    fn key_bound_values(&self) -> &'static [&'static str] {
        &[]
    }

    async fn known_values(&self) -> Result<Option<Vec<String>>, SubjectAttributeError> {
        Ok(None)
    }
}
