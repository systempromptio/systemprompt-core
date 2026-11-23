//! Authorization subject types shared across layers.
//!
//! The [`SubjectAttributes`] value type and the [`SubjectAttributeSource`]
//! seam extensions read them through. The rule-type vocabulary and the
//! subject dimension live in `systemprompt_traits::authz`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::collections::BTreeMap;
use systemprompt_identifiers::UserId;
use systemprompt_provider_contracts::governance::RuleType;
use systemprompt_traits::authz::SubjectDimension;

mod handle;

pub use handle::SubjectSourceHandle;

/// Subject values per dimension, gathered before the pure resolver runs.
///
/// A dimension with no values for the user is simply absent, which makes its
/// rules unmatchable for that request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubjectAttributes(BTreeMap<RuleType, Vec<String>>);

impl SubjectAttributes {
    pub const EMPTY: Self = Self(BTreeMap::new());

    #[must_use]
    pub const fn new() -> Self {
        Self::EMPTY
    }

    pub fn insert(&mut self, rule_type: RuleType, values: Vec<String>) {
        self.0.insert(rule_type, values);
    }

    #[must_use]
    pub fn values(&self, rule_type: &RuleType) -> &[String] {
        self.0.get(rule_type).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&RuleType, &[String])> {
        self.0
            .iter()
            .map(|(rule_type, values)| (rule_type, values.as_slice()))
    }
}

impl FromIterator<(RuleType, Vec<String>)> for SubjectAttributes {
    fn from_iter<I: IntoIterator<Item = (RuleType, Vec<String>)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SubjectSourceError {
    #[error("the subject attribute source is not ready")]
    NotReady,
    #[error("no subject dimension '{0}' is registered")]
    UnknownDimension(String),
    #[error("subject dimension '{dimension}' could not be read")]
    Provider {
        dimension: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

/// Reads the values a user holds per extension-declared subject dimension.
///
/// `known_values` lists every value a dimension can take when its provider
/// can enumerate them, and answers `None` when it cannot; a caller treats
/// `None` as "any value may exist", never as "no value exists".
///
/// Held as `Arc<dyn SubjectAttributeSource>` by the extension context and the
/// handle below, so it uses `#[async_trait]`: native `async fn` in traits is
/// not `dyn`-compatible.
#[async_trait]
pub trait SubjectAttributeSource: Send + Sync {
    fn dimensions(&self) -> &[SubjectDimension];

    async fn values_for(
        &self,
        user: &UserId,
        dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError>;

    async fn resolve_all(&self, user: &UserId) -> Result<SubjectAttributes, SubjectSourceError>;

    async fn known_values(
        &self,
        _dimension: &SubjectDimension,
    ) -> Result<Option<Vec<String>>, SubjectSourceError> {
        Ok(None)
    }
}
