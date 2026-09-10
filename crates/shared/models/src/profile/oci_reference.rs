//! Parser for the `registry/repository[:tag|@sha256:<digest>]` references used
//! by OCI services-bundle sources.
//!
//! The registry host is mandatory: an unqualified reference such as
//! `alpine:3` would otherwise resolve against an implicit default registry,
//! which is not a decision a profile should make silently.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciReference {
    pub registry: String,
    pub repository: String,
    pub tag: Option<String>,
    pub digest: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OciReferenceError {
    #[error("OCI reference is empty")]
    Empty,

    #[error("OCI reference must be registry/repository[:tag|@sha256:...], got: {0}")]
    MissingRegistry(String),

    #[error("OCI reference has an empty repository: {0}")]
    EmptyRepository(String),

    #[error("OCI repository path segment is not [a-z0-9] with . _ - separators: {0}")]
    InvalidRepository(String),

    #[error("OCI digest must be sha256:<64 lowercase hex>, got: {0}")]
    InvalidDigest(String),

    #[error("OCI tag must be [A-Za-z0-9_][A-Za-z0-9._-]{{0,127}}, got: {0}")]
    InvalidTag(String),
}

impl OciReference {
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        self.digest.is_some()
    }
}

fn is_registry_host(segment: &str) -> bool {
    segment == "localhost" || segment.contains('.') || segment.contains(':')
}

fn validate_repository(repository: &str) -> Result<(), OciReferenceError> {
    let invalid = || OciReferenceError::InvalidRepository(repository.to_owned());
    for segment in repository.split('/') {
        if segment.is_empty() {
            return Err(invalid());
        }
        let edge_ok = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
        let first_ok = segment.chars().next().is_some_and(edge_ok);
        let last_ok = segment.chars().next_back().is_some_and(edge_ok);
        let body_ok = segment
            .chars()
            .all(|c| edge_ok(c) || matches!(c, '.' | '_' | '-'));
        if !(first_ok && last_ok && body_ok) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn validate_tag(tag: &str) -> Result<(), OciReferenceError> {
    let invalid = || OciReferenceError::InvalidTag(tag.to_owned());
    if tag.is_empty() || tag.len() > 128 {
        return Err(invalid());
    }
    let first_ok = tag
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    let body_ok = tag
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if first_ok && body_ok {
        Ok(())
    } else {
        Err(invalid())
    }
}

fn validate_digest(digest: &str) -> Result<(), OciReferenceError> {
    let hex = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| OciReferenceError::InvalidDigest(digest.to_owned()))?;
    let lower_hex = hex
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if hex.len() == 64 && lower_hex {
        Ok(())
    } else {
        Err(OciReferenceError::InvalidDigest(digest.to_owned()))
    }
}

impl FromStr for OciReference {
    type Err = OciReferenceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = s.strip_prefix("oci://").unwrap_or(s);
        if raw.is_empty() {
            return Err(OciReferenceError::Empty);
        }

        let (name, digest) = match raw.split_once('@') {
            Some((name, digest)) => {
                validate_digest(digest)?;
                (name, Some(digest.to_owned()))
            },
            None => (raw, None),
        };

        let (name, tag) = match name.rfind(':') {
            Some(idx) if !name[idx + 1..].contains('/') => {
                let tag = &name[idx + 1..];
                validate_tag(tag)?;
                (&name[..idx], Some(tag.to_owned()))
            },
            _ => (name, None),
        };

        let (registry, repository) = name
            .split_once('/')
            .ok_or_else(|| OciReferenceError::MissingRegistry(s.to_owned()))?;

        if !is_registry_host(registry) {
            return Err(OciReferenceError::MissingRegistry(s.to_owned()));
        }
        if repository.is_empty() {
            return Err(OciReferenceError::EmptyRepository(s.to_owned()));
        }
        validate_repository(repository)?;

        Ok(Self {
            registry: registry.to_owned(),
            repository: repository.to_owned(),
            tag,
            digest,
        })
    }
}

impl fmt::Display for OciReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.registry, self.repository)?;
        if let Some(tag) = &self.tag {
            write!(f, ":{tag}")?;
        }
        if let Some(digest) = &self.digest {
            write!(f, "@{digest}")?;
        }
        Ok(())
    }
}
