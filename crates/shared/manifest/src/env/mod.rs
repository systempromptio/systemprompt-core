//! Environment-variable reading and `${VAR}` / `${VAR:-default}` interpolation.
//!
//! Two configuration layers are interpolated: the profile (driven by
//! [`crate::profile::profile_field_kind`], read from the profile's JSON
//! schema) and the services tree (driven by
//! [`crate::services::interpolation::services_field_kind`]). Interpolation
//! runs over a parsed YAML tree, never over document text: only string
//! scalars are rewritten, so a substituted value can never add a key, end a
//! scalar early or start a comment. How a scalar that is exactly one
//! placeholder enters the tree is decided by the type of the field it lands
//! in ([`FieldKind`]), never by the value's appearance: a field that accepts a
//! string receives the value as a string, a numeric or boolean field receives
//! it parsed, and an empty value is null where the field accepts null. A
//! placeholder embedded in longer text always yields a string. A field whose
//! kind is [`FieldKind::VERBATIM`] is never rewritten. A placeholder whose
//! variable is unset (or empty) and has no `:-` default is an
//! [`InterpolationError`] naming the variable and the key path
//! ([`interpolate_yaml`], for configuration about to run), is recorded and
//! left in the tree as a typed `!unresolved` marker
//! ([`interpolate_yaml_available`], for a profile that is only inspected,
//! edited or shipped from a host that lacks the target's variables), or is
//! recorded and left as its original placeholder text
//! ([`interpolate_yaml_keeping_unresolved`], for an inspected services tree,
//! whose string fields must still deserialize).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::LazyLock;

use regex::Regex;
use thiserror::Error;

mod tree;

pub use tree::{interpolate_yaml, interpolate_yaml_available, interpolate_yaml_keeping_unresolved};

#[expect(
    clippy::expect_used,
    reason = "compile-time-constant regex; failure is a programmer bug, not runtime input"
)]
static INTERPOLATION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([^}:]+)(?::-(.*?))?\}")
        .expect("INTERPOLATION_REGEX is a valid regex - this is a compile-time constant")
});

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("environment variable `{variable}` is not set and its placeholder has no `:-` default")]
pub struct UnsetVariable {
    pub variable: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "`{key_path}` references environment variable `{variable}`, which is not set; set it or \
     write `${{{variable}:-<default>}}`"
)]
pub struct InterpolationError {
    pub variable: String,
    pub key_path: String,
}

#[must_use]
pub fn read_env_optional(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Some(v),
        Ok(_) | Err(_) => None,
    }
}

#[must_use]
pub fn none_if_blank(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

#[must_use]
pub fn contains_placeholder(input: &str) -> bool {
    INTERPOLATION_REGEX.is_match(input)
}

pub fn interpolate(
    input: &str,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<String, UnsetVariable> {
    let mut unset = None;
    let output = INTERPOLATION_REGEX.replace_all(input, |caps: &regex::Captures| {
        let var_name = &caps[1];
        lookup(var_name)
            .or_else(|| caps.get(2).map(|m| m.as_str().to_owned()))
            .unwrap_or_else(|| {
                unset.get_or_insert_with(|| var_name.to_owned());
                String::new()
            })
    });
    unset.map_or_else(
        || Ok(output.into_owned()),
        |variable| Err(UnsetVariable { variable }),
    )
}

/// The scalar types a key path accepts, as a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldKind(u8);

impl FieldKind {
    pub const NONE: Self = Self(0);
    pub const STRING: Self = Self(1);
    pub const NUMBER: Self = Self(2);
    pub const BOOL: Self = Self(4);
    pub const NULL: Self = Self(8);
    pub const UNKNOWN: Self = Self(16);
    pub const VERBATIM: Self = Self(32);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn accepts(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[must_use]
    pub const fn accepts_string(self) -> bool {
        self.accepts(Self::STRING) || self.accepts(Self::UNKNOWN)
    }
}

/// One step of a key path: a mapping key or a sequence index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Key(String),
    Index(usize),
}

pub type FieldKindLookup<'a> = &'a dyn Fn(&[PathSegment]) -> FieldKind;

#[must_use]
pub const fn untyped(_path: &[PathSegment]) -> FieldKind {
    FieldKind::UNKNOWN
}

pub const UNRESOLVED_TAG: &str = "!unresolved";

#[must_use]
pub fn key_path_label(path: &[PathSegment]) -> String {
    let mut label = String::new();
    for segment in path {
        match segment {
            PathSegment::Key(key) => {
                if !label.is_empty() {
                    label.push('.');
                }
                label.push_str(key);
            },
            PathSegment::Index(index) => label.push_str(&format!("[{index}]")),
        }
    }
    if label.is_empty() {
        "<root>".to_owned()
    } else {
        label
    }
}
