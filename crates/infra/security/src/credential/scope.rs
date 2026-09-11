//! What a credential knows about *where* it may be spent, and how it is sent.
//!
//! A credential is not only a secret: it also carries the coordinates the
//! endpoint needs. A Google service-account key names the one Cloud project
//! any token minted from it can address; a future workload identity will name
//! a region or an account. [`CredentialScope`] is that set of coordinates,
//! and [`fill_endpoint`] is the only place a catalog endpoint template is
//! resolved against them — so an endpoint asking for a coordinate the
//! credential does not carry is refused rather than guessed at.
//!
//! [`fill_endpoint`] is the generalisation of what used to be a `{project}`
//! substitution: the catalog never carries a tenant identifier, so a secret
//! swapped for another customer's re-targets the endpoint with it. An
//! endpoint that asks for a coordinate this credential cannot supply cannot be
//! served — guessing one would send the request somewhere the operator never
//! chose. `PLACEHOLDERS` is every placeholder a template may ask for.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use super::error::CredentialError;

pub use systemprompt_models::services::providers::{PROJECT_PLACEHOLDER, REGION_PLACEHOLDER};

/// How an upstream expects the credential to be presented: as
/// `Authorization: Bearer <token>` (a minted, expiring OAuth token) or in the
/// provider's own API-key header, sent verbatim by the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    Bearer,
    ApiKey,
}

/// The credential, resolved into the exact value an adapter will send.
///
/// `Debug` is hand-written: this type exists to be logged near, never logged.
#[derive(Clone)]
pub struct AuthHeader {
    pub scheme: AuthScheme,
    pub value: String,
}

impl AuthHeader {
    #[must_use]
    pub const fn is_bearer(&self) -> bool {
        matches!(self.scheme, AuthScheme::Bearer)
    }
}

impl fmt::Debug for AuthHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthHeader")
            .field("scheme", &self.scheme)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// The coordinates a credential supplies to the endpoint it authenticates.
///
/// The cloud project, account or tenant it is confined to; the region, when
/// it is confined to one; and the principal it acts as, for audit — never a
/// secret.
///
/// Every field is optional because most credentials supply none of them: an
/// API key is a bare string and names nothing (`empty`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CredentialScope {
    pub project: Option<String>,
    pub region: Option<String>,
    pub principal: Option<String>,
}

impl CredentialScope {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            project: None,
            region: None,
            principal: None,
        }
    }

    fn value_for(&self, placeholder: &str) -> Option<&str> {
        let value = match placeholder {
            PROJECT_PLACEHOLDER => self.project.as_deref(),
            REGION_PLACEHOLDER => self.region.as_deref(),
            _ => None,
        };
        // Why: an empty coordinate is no coordinate. Substituting one would
        // produce `/projects//locations/...`, which reaches the upstream and
        // fails there with an error about a path rather than a credential.
        value.filter(|v| !v.is_empty())
    }
}

const PLACEHOLDERS: &[(&str, &str)] = &[
    (PROJECT_PLACEHOLDER, "project id"),
    (REGION_PLACEHOLDER, "region"),
];

pub fn fill_endpoint(template: &str, scope: &CredentialScope) -> Result<String, CredentialError> {
    let mut endpoint = template.to_owned();
    for &(placeholder, field) in PLACEHOLDERS {
        if !endpoint.contains(placeholder) {
            continue;
        }
        let Some(value) = scope.value_for(placeholder) else {
            return Err(CredentialError::MissingScope {
                endpoint: template.to_owned(),
                field,
                placeholder,
            });
        };
        endpoint = endpoint.replace(placeholder, value);
    }
    Ok(endpoint)
}
