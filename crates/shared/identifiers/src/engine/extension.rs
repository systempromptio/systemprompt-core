//! The id of an extension: a short lowercase name such as `drive` or
//! `brain`, shared by an extension's engine, instance and shell halves and
//! named by an instance when it enables the extension for this host.
//!
//! An id is 1 to 64 characters of `a-z`, `0-9` and single hyphens, starting
//! with a letter and not ending with a hyphen. Ids read from a manifest are
//! validated with `parse`; an extension names its own id with
//! [`extension_id!`](crate::engine::extension_id), which checks the literal
//! while compiling, so a malformed id fails the build and the result needs no
//! error handling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use crate::engine::error::IdError;

const KIND: &str = "extension";

/// The id of an extension.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String")]
pub struct ExtensionId(Cow<'static, str>);

impl ExtensionId {
    pub const MAX_LEN: usize = 64;

    pub fn parse(value: &str) -> Result<Self, IdError> {
        if Self::is_well_formed(value) {
            Ok(Self(Cow::Owned(value.to_owned())))
        } else {
            Err(malformed(value.to_owned()))
        }
    }

    pub const fn is_well_formed(value: &str) -> bool {
        let bytes = value.as_bytes();
        if bytes.is_empty() || bytes.len() > Self::MAX_LEN || !bytes[0].is_ascii_lowercase() {
            return false;
        }
        let mut previous = bytes[0];
        let mut index = 1;
        while index < bytes.len() {
            let byte = bytes[index];
            let allowed = byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (byte == b'-' && previous != b'-');
            if !allowed {
                return false;
            }
            previous = byte;
            index += 1;
        }
        previous != b'-'
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A string literal already checked to be a well-formed extension id; it has no
/// destructor, so the check runs in a `const`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtensionLiteral(&'static str);

impl ExtensionLiteral {
    pub const fn new(value: &'static str) -> Option<Self> {
        if ExtensionId::is_well_formed(value) {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn id(self) -> ExtensionId {
        ExtensionId(Cow::Borrowed(self.0))
    }
}

const fn malformed(value: String) -> IdError {
    IdError { kind: KIND, value }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ExtensionId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, IdError> {
        Self::parse(value)
    }
}

impl TryFrom<String> for ExtensionId {
    type Error = IdError;

    fn try_from(value: String) -> Result<Self, IdError> {
        if Self::is_well_formed(&value) {
            Ok(Self(Cow::Owned(value)))
        } else {
            Err(malformed(value))
        }
    }
}

impl AsRef<str> for ExtensionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[macro_export]
macro_rules! extension_id {
    ($value:literal) => {{
        const LITERAL: $crate::engine::ExtensionLiteral =
            match $crate::engine::ExtensionLiteral::new($value) {
                ::core::option::Option::Some(literal) => literal,
                ::core::option::Option::None => ::core::panic!("not a well-formed extension id"),
            };
        LITERAL.id()
    }};
}
