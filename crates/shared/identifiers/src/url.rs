//! Validated URL type.

use crate::error::IdValidationError;
use crate::{DbValue, ToDbValue};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct ValidatedUrl(String);

impl ValidatedUrl {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdValidationError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdValidationError::empty("ValidatedUrl"));
        }
        let scheme_end = value.find("://").ok_or_else(|| {
            IdValidationError::invalid("ValidatedUrl", "must have a scheme (e.g., 'https://')")
        })?;
        let scheme = &value[..scheme_end];
        if scheme.is_empty() {
            return Err(IdValidationError::invalid(
                "ValidatedUrl",
                "scheme cannot be empty",
            ));
        }
        if !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
        {
            return Err(IdValidationError::invalid(
                "ValidatedUrl",
                "scheme contains invalid characters",
            ));
        }
        if !scheme.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return Err(IdValidationError::invalid(
                "ValidatedUrl",
                "scheme must start with a letter",
            ));
        }
        let after_scheme = &value[scheme_end + 3..];
        if after_scheme.is_empty() {
            return Err(IdValidationError::invalid(
                "ValidatedUrl",
                "URL must have a host component",
            ));
        }
        let host_end = after_scheme.find('/').unwrap_or(after_scheme.len());
        let authority = &after_scheme[..host_end];
        let host_part = authority
            .rfind('@')
            .map_or(authority, |i| &authority[i + 1..]);

        let host = if host_part.starts_with('[') {
            let bracket_end = host_part.find(']').ok_or_else(|| {
                IdValidationError::invalid("ValidatedUrl", "IPv6 address missing closing bracket")
            })?;
            &host_part[..=bracket_end]
        } else {
            host_part.split(':').next().unwrap_or(host_part)
        };

        if host.starts_with('[') && host.ends_with(']') {
            let ipv6_content = &host[1..host.len() - 1];
            if ipv6_content.is_empty() {
                return Err(IdValidationError::invalid(
                    "ValidatedUrl",
                    "IPv6 address cannot be empty",
                ));
            }
        }

        if host_part.contains("]:") || (!host_part.starts_with('[') && host_part.contains(':')) {
            let port_part = if host_part.starts_with('[') {
                host_part.rsplit("]:").next()
            } else {
                host_part.split(':').nth(1)
            };
            if let Some(port) = port_part {
                if port.is_empty() || port.starts_with('/') {
                    return Err(IdValidationError::invalid(
                        "ValidatedUrl",
                        "port cannot be empty when ':' is present",
                    ));
                }
            }
        }

        if host.is_empty() && !scheme.eq_ignore_ascii_case("file") {
            return Err(IdValidationError::invalid(
                "ValidatedUrl",
                "host cannot be empty",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new(value: impl Into<String>) -> Self {
        Self::try_new(value).expect("ValidatedUrl validation failed")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0.split("://").next().unwrap_or("")
    }

    #[must_use]
    pub fn is_https(&self) -> bool {
        self.scheme().eq_ignore_ascii_case("https")
    }

    #[must_use]
    pub fn is_http(&self) -> bool {
        let scheme = self.scheme().to_ascii_lowercase();
        scheme == "http" || scheme == "https"
    }
}

impl fmt::Display for ValidatedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for ValidatedUrl {
    type Error = IdValidationError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for ValidatedUrl {
    type Error = IdValidationError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl std::str::FromStr for ValidatedUrl {
    type Err = IdValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl AsRef<str> for ValidatedUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ValidatedUrl {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(s).map_err(serde::de::Error::custom)
    }
}

impl ToDbValue for ValidatedUrl {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}

impl ToDbValue for &ValidatedUrl {
    fn to_db_value(&self) -> DbValue {
        DbValue::String(self.0.clone())
    }
}
