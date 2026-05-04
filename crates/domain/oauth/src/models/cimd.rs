//! Client-Initiated Metadata Discovery (CIMD) value objects.
//!
//! `CimdMetadata` is the deserialised JSON document fetched from a
//! federated client's well-known URL. `ClientValidation` enumerates the
//! validation paths a client may take (DCR, CIMD, first-party, system).

use crate::error::{OauthError, OauthResult as Result};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ClientId, ClientType};

/// CIMD client metadata as published at the client's well-known endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CimdMetadata {
    /// Federated client identifier (must be an HTTPS URL).
    pub client_id: ClientId,
    /// Optional human-readable client name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    /// Permitted redirect URIs.
    pub redirect_uris: Vec<String>,
    /// OAuth grant types the client supports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_types: Option<Vec<String>>,
    /// OAuth response types the client supports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_types: Option<Vec<String>>,
    /// Token endpoint authentication method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_method: Option<String>,
    /// URL of the client's logo asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_uri: Option<String>,
    /// URL of the client's home/landing page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_uri: Option<String>,
    /// Contact addresses for the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contacts: Option<Vec<String>>,
}

impl CimdMetadata {
    /// Validate that the metadata document conforms to CIMD constraints.
    ///
    /// Enforces HTTPS `client_id`, non-empty `redirect_uris`, and rejects
    /// redirect URIs containing path-traversal sequences or NUL bytes.
    pub fn validate(&self) -> Result<()> {
        if !self.client_id.as_str().starts_with("https://") {
            return Err(OauthError::Validation(
                "client_id must be HTTPS URL".to_string(),
            ));
        }

        if self.redirect_uris.is_empty() {
            return Err(OauthError::Validation(
                "redirect_uris cannot be empty".to_string(),
            ));
        }

        for uri in &self.redirect_uris {
            if uri.contains("..") || uri.contains('\0') {
                return Err(OauthError::Validation(format!(
                    "Invalid redirect_uri: {uri}"
                )));
            }
        }

        Ok(())
    }

    /// Returns `true` if `uri` exactly matches one of the registered redirect
    /// URIs.
    pub fn has_redirect_uri(&self, uri: &str) -> bool {
        self.redirect_uris.iter().any(|u| u == uri)
    }
}

/// Outcome of resolving how to validate a given OAuth client.
#[derive(Debug)]
pub enum ClientValidation {
    /// Client was registered via Dynamic Client Registration.
    Dcr {
        /// Registered client identifier.
        client_id: ClientId,
    },
    /// Client metadata was discovered via CIMD.
    Cimd {
        /// CIMD client identifier.
        client_id: ClientId,
        /// Discovered metadata document.
        metadata: Box<CimdMetadata>,
    },
    /// Client is a first-party application built into the platform.
    FirstParty {
        /// First-party client identifier.
        client_id: ClientId,
    },
    /// Client is a system / service-account client.
    System {
        /// System client identifier.
        client_id: ClientId,
    },
}

impl ClientValidation {
    /// Returns the client identifier regardless of validation flavour.
    pub const fn client_id(&self) -> &ClientId {
        match self {
            Self::Cimd { client_id, .. }
            | Self::Dcr { client_id }
            | Self::FirstParty { client_id }
            | Self::System { client_id } => client_id,
        }
    }

    /// Returns the canonical [`ClientType`] for this validation outcome.
    pub fn client_type(&self) -> ClientType {
        match self {
            Self::Dcr { client_id } => client_id.client_type(),
            Self::Cimd { .. } => ClientType::Cimd,
            Self::FirstParty { .. } => ClientType::FirstParty,
            Self::System { .. } => ClientType::System,
        }
    }
}
