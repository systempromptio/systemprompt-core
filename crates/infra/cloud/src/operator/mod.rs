//! Operator identity for self-managed tenants.
//!
//! A self-managed tenant is not reachable through the systemprompt.io
//! control plane, so its operators sign in against an OIDC issuer the
//! tenant trusts (`security.trusted_issuers` on the tenant).
//! [`run_operator_login`] runs the browser authorization-code flow with PKCE
//! over the loopback callback and stores the issuer's ID token as
//! [`OperatorCredentials`]; [`exchange_for_tenant`] trades it for a tenant
//! session through RFC 8693 token exchange at the tenant's own token endpoint.
//! The ID token's signature is verified by the tenant during that exchange, not
//! here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod endpoints;
mod login;

use std::fs;
use std::path::Path;

use base64::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CloudAuthToken, Email};

use crate::error::{CloudError, CloudResult};
use crate::private_dir::write_private_json;
use crate::{CloudApiClient, auth};

pub use login::{OperatorLoginRequest, run_operator_login};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorCredentials {
    pub issuer: String,

    pub client_id: String,

    pub id_token: CloudAuthToken,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<Email>,

    pub authenticated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct IdentityClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    nonce: Option<String>,
}

impl OperatorCredentials {
    #[must_use]
    pub fn is_expired(&self) -> bool {
        auth::is_expired(&self.id_token)
    }

    pub fn load_from_path(path: &Path) -> CloudResult<Self> {
        if !path.exists() {
            return Err(CloudError::OperatorNotAuthenticated);
        }
        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|e| CloudError::CredentialsCorrupted {
            source: Box::new(e),
        })
    }

    pub fn load_valid(path: &Path) -> CloudResult<Self> {
        let creds = Self::load_from_path(path)?;
        if creds.is_expired() {
            return Err(CloudError::OperatorNotAuthenticated);
        }
        Ok(creds)
    }

    pub fn save_to_path(&self, path: &Path) -> CloudResult<()> {
        write_private_json(path, self)
    }

    pub fn delete_from_path(path: &Path) -> CloudResult<()> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

fn identity_claims(id_token: &str) -> CloudResult<IdentityClaims> {
    let payload = id_token.split('.').nth(1).ok_or(CloudError::JwtDecode)?;
    let raw = BASE64_URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_e| CloudError::JwtDecode)?;
    serde_json::from_slice(&raw).map_err(|_e| CloudError::JwtDecode)
}

#[derive(Debug, Clone)]
pub struct TenantSession {
    pub access_token: String,
}

pub async fn exchange_for_tenant(
    tenant_base_url: &str,
    operator: &OperatorCredentials,
) -> CloudResult<TenantSession> {
    let client = CloudApiClient::new(tenant_base_url, operator.id_token.as_str())?;
    let access_token = client.tenant_session_token().await?;
    Ok(TenantSession { access_token })
}
