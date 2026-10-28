//! Authorization subject types shared across layers.
//!
//! The open [`RuleType`] vocabulary, the [`SubjectDimension`] and
//! [`SubjectAttributes`] value types, and the [`SubjectAttributeSource`] seam
//! extensions read them through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

#[derive(Debug, thiserror::Error)]
#[error("invalid rule_type: {0}")]
pub struct RuleTypeError(pub String);

/// Open subject-dimension vocabulary bound to `access_control_rules.rule_type`.
///
/// Core mints exactly two: [`RuleType::USER`] and [`RuleType::ROLE`]. Every
/// other dimension (department, cost centre, clearance, ...) is an extension
/// concern, minted with [`RuleType::extension`] and taught to the resolver via
/// a [`SubjectDimension`] registered by
/// `register_subject_attribute_provider!`. Core never interprets an
/// extension slug. [`RuleType::extension_static`] mints a literal slug in a
/// `const`, so a malformed one fails the build instead of a runtime check.
///
/// This mirrors the authz context: the column is an open vocabulary
/// validated at the Rust boundary rather than by a SQL `CHECK`, so an
/// unrecognised-but-well-formed slug is data, not an error.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleType(Cow<'static, str>);

impl RuleType {
    pub const USER: Self = Self(Cow::Borrowed("user"));
    pub const ROLE: Self = Self(Cow::Borrowed("role"));

    pub fn extension(slug: impl Into<Cow<'static, str>>) -> Result<Self, RuleTypeError> {
        let slug = slug.into();
        if !is_extension_slug(slug.as_bytes()) {
            return Err(RuleTypeError(slug.into_owned()));
        }
        Ok(Self(slug))
    }

    #[must_use]
    pub const fn extension_static(slug: &'static str) -> Self {
        assert!(
            is_extension_slug(slug.as_bytes()),
            "extension rule type must be [a-z0-9_], not edged by `_`, and not `user` or `role`"
        );
        Self(Cow::Borrowed(slug))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

const fn is_extension_slug(slug: &[u8]) -> bool {
    if matches!(slug, [] | [b'_', ..] | [.., b'_'])
        || bytes_eq(slug, b"user")
        || bytes_eq(slug, b"role")
    {
        return false;
    }
    let mut rest = slug;
    while let [byte, tail @ ..] = rest {
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_') {
            return false;
        }
        rest = tail;
    }
    true
}

const fn bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let (mut left, mut right) = (left, right);
    while let ([a, left_tail @ ..], [b, right_tail @ ..]) = (left, right) {
        if *a != *b {
            return false;
        }
        left = left_tail;
        right = right_tail;
    }
    true
}

impl fmt::Display for RuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for RuleType {
    fn from(s: &str) -> Self {
        match s {
            "user" => Self::USER,
            "role" => Self::ROLE,
            other => Self(Cow::Owned(other.to_owned())),
        }
    }
}

#[cfg(feature = "sqlx")]
impl sqlx::Type<sqlx::Postgres> for RuleType {
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        <str as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &<sqlx::Postgres as sqlx::Database>::TypeInfo) -> bool {
        <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "sqlx")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for RuleType {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode(self.as_str(), buf)
    }
}

#[cfg(feature = "sqlx")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for RuleType {
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let raw = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        Ok(Self::from(raw))
    }
}

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

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("the subject attribute source is already bound")]
pub struct AlreadyBound;

/// A subject source bound after the providers that read it are built.
///
/// Providers are discovered from a context that must already carry a source,
/// and one provider may itself consume the set. The handle breaks that cycle:
/// it is created first, handed to every factory, and bound once the set exists.
/// Before binding every read fails with [`SubjectSourceError::NotReady`]
/// rather than answering from an empty set.
#[derive(Clone, Default)]
pub struct SubjectSourceHandle(Arc<OnceLock<Arc<dyn SubjectAttributeSource>>>);

impl fmt::Debug for SubjectSourceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubjectSourceHandle")
            .field("bound", &self.0.get().is_some())
            .finish()
    }
}

impl SubjectSourceHandle {
    pub fn bind(&self, source: Arc<dyn SubjectAttributeSource>) -> Result<(), AlreadyBound> {
        self.0.set(source).map_err(|_unbound| AlreadyBound)
    }
}

#[async_trait]
impl SubjectAttributeSource for SubjectSourceHandle {
    fn dimensions(&self) -> &[SubjectDimension] {
        self.0.get().map_or(&[], |source| source.dimensions())
    }

    async fn values_for(
        &self,
        user: &UserId,
        dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError> {
        match self.0.get() {
            Some(source) => source.values_for(user, dimension).await,
            None => Err(SubjectSourceError::NotReady),
        }
    }

    async fn resolve_all(&self, user: &UserId) -> Result<SubjectAttributes, SubjectSourceError> {
        match self.0.get() {
            Some(source) => source.resolve_all(user).await,
            None => Err(SubjectSourceError::NotReady),
        }
    }

    async fn known_values(
        &self,
        dimension: &SubjectDimension,
    ) -> Result<Option<Vec<String>>, SubjectSourceError> {
        match self.0.get() {
            Some(source) => source.known_values(dimension).await,
            None => Err(SubjectSourceError::NotReady),
        }
    }
}
