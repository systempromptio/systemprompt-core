//! Pure ID-JAG claim validation, shared by the EMA issuer and resource-server
//! paths. Signature, JWKS, and `jti` replay are the caller's job.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ClientId;

pub const ID_JAG_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id-jag";

/// Mandatory JOSE `typ` header of an ID-JAG (draft §3).
pub const ID_JAG_TYP: &str = "oauth-id-jag+jwt";

pub const DEFAULT_LEEWAY_SECS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdJagClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<ClientId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub azp: Option<ClientId>,
    pub jti: String,
    pub exp: i64,
    pub iat: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl IdJagClaims {
    /// `client_id`, falling back to `azp` (draft permits either).
    #[must_use]
    pub fn bound_client(&self) -> Option<&ClientId> {
        self.client_id.as_ref().or(self.azp.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdJagError {
    #[error("ID-JAG must carry JOSE typ '{ID_JAG_TYP}', found {found:?}")]
    WrongTyp { found: Option<String> },
    #[error("ID-JAG aud '{found}' is not this resource '{expected}'")]
    AudienceMismatch { expected: String, found: String },
    #[error("ID-JAG missing client_id/azp binding")]
    MissingClient,
    #[error("ID-JAG client '{found}' does not match authenticated client '{expected}'")]
    ClientMismatch { expected: String, found: String },
    #[error("ID-JAG client '{found}' is not an allowed client for its issuer")]
    ClientNotAllowed { found: String },
    #[error("ID-JAG has expired")]
    Expired,
    #[error("ID-JAG iat is in the future")]
    IssuedInFuture,
}

pub fn validate_typ(typ: Option<&str>) -> Result<(), IdJagError> {
    match typ {
        Some(t) if t == ID_JAG_TYP => Ok(()),
        other => Err(IdJagError::WrongTyp {
            found: other.map(ToOwned::to_owned),
        }),
    }
}

#[derive(Debug)]
pub struct ClaimPolicy<'a> {
    pub expected_audience: &'a str,
    pub authenticated_client: &'a str,
    /// Empty means any client (still bound to `authenticated_client`).
    pub allowed_client_ids: &'a [String],
    pub now: i64,
    pub leeway: i64,
}

pub fn validate_claims(claims: &IdJagClaims, policy: &ClaimPolicy<'_>) -> Result<(), IdJagError> {
    if claims.aud != policy.expected_audience {
        return Err(IdJagError::AudienceMismatch {
            expected: policy.expected_audience.to_owned(),
            found: claims.aud.clone(),
        });
    }

    let bound = claims.bound_client().ok_or(IdJagError::MissingClient)?;
    if bound.as_str() != policy.authenticated_client {
        return Err(IdJagError::ClientMismatch {
            expected: policy.authenticated_client.to_owned(),
            found: bound.as_str().to_owned(),
        });
    }
    if !policy.allowed_client_ids.is_empty()
        && !policy
            .allowed_client_ids
            .iter()
            .any(|c| c.as_str() == bound.as_str())
    {
        return Err(IdJagError::ClientNotAllowed {
            found: bound.as_str().to_owned(),
        });
    }

    if claims.exp <= policy.now - policy.leeway {
        return Err(IdJagError::Expired);
    }
    if claims.iat > policy.now + policy.leeway {
        return Err(IdJagError::IssuedInFuture);
    }

    Ok(())
}
