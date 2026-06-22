//! Subject-token validation for token exchange.
//!
//! Resolves the `subject_token` to a verified identity, routing self-issued
//! tokens through the local signing authority and federated tokens through the
//! issuer's JWKS. The `iss` peeked from the unsigned payload only selects the
//! verification path; issuer and signature are re-validated downstream.

use anyhow::{Result, anyhow};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use systemprompt_models::Config;
use systemprompt_models::auth::{ActClaim, JwtAudience, JwtClaims, Permission};
use systemprompt_models::profile::TrustedIssuer;
use systemprompt_security::keys::JwksClient;

use super::super::super::TokenError;
use super::{ACCESS_TOKEN_TYPE, ID_TOKEN_TYPE, JWT_TOKEN_TYPE};

#[derive(serde::Deserialize)]
struct IssOnly {
    iss: String,
}

pub(super) struct SubjectIdentity {
    pub(super) scope: Vec<Permission>,
    pub(super) prior_act: Option<ActClaim>,
}

impl SubjectIdentity {
    pub(super) const fn new(scope: Vec<Permission>) -> Self {
        Self {
            scope,
            prior_act: None,
        }
    }
}

pub(super) async fn validate_subject_token(
    token: &str,
    token_type: &str,
    global: &Config,
) -> Result<SubjectIdentity> {
    if !matches!(
        token_type,
        ACCESS_TOKEN_TYPE | ID_TOKEN_TYPE | JWT_TOKEN_TYPE
    ) {
        return Err(anyhow!(TokenError::InvalidRequest {
            field: "subject_token_type".to_owned(),
            message: format!("unsupported subject_token_type '{token_type}'"),
        }));
    }

    let header = decode_header(token).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("malformed JWT header: {e}"),
        })
    })?;

    let declared_iss = peek_issuer(token)?;

    if declared_iss == global.jwt_issuer {
        return validate_self_issued(token, global);
    }

    let trusted = global
        .trusted_issuers
        .iter()
        .find(|t| t.issuer == declared_iss)
        .ok_or_else(|| {
            anyhow!(TokenError::InvalidRequest {
                field: "subject_token".to_owned(),
                message: format!("issuer '{declared_iss}' is not trusted"),
            })
        })?;

    let kid = header.kid.ok_or_else(|| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: "trusted-issuer token must carry a kid header".to_owned(),
        })
    })?;

    let allowed_hosts = jwks_host_allowlist(&global.trusted_issuers);
    let client = JwksClient::new(allowed_hosts);
    let jwk = client
        .fetch_at(&trusted.issuer, &trusted.jwks_uri, &kid)
        .await
        .map_err(|e| {
            anyhow!(TokenError::InvalidRequest {
                field: "subject_token".to_owned(),
                message: format!("JWKS resolution failed: {e}"),
            })
        })?;

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("invalid RSA components in JWK: {e}"),
        })
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&trusted.issuer]);
    validation.set_audience(&[&trusted.audience]);

    let data = decode::<JwtClaims>(token, &decoding_key, &validation).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("subject token signature/claims rejected: {e}"),
        })
    })?;

    Ok(SubjectIdentity {
        scope: data.claims.scope,
        prior_act: data.claims.act,
    })
}

pub fn peek_issuer(token: &str) -> Result<String> {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let mut parts = token.split('.');
    let _header = parts.next();
    let payload = parts.next().ok_or_else(|| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: "subject_token is not a JWT".to_owned(),
        })
    })?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("subject_token payload is not base64url: {e}"),
        })
    })?;
    let parsed: IssOnly = serde_json::from_slice(&bytes).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("subject_token payload missing iss: {e}"),
        })
    })?;
    Ok(parsed.iss)
}

fn validate_self_issued(token: &str, global: &Config) -> Result<SubjectIdentity> {
    use jsonwebtoken::decode_header;
    use systemprompt_security::keys::authority;

    let header = decode_header(token).map_err(|e| {
        anyhow!(TokenError::InvalidGrant {
            reason: format!("subject_token header decode failed: {e}"),
        })
    })?;
    if header.alg != Algorithm::RS256 {
        return Err(anyhow!(TokenError::InvalidGrant {
            reason: "subject_token must be RS256-signed".to_owned(),
        }));
    }
    let kid = header.kid.as_deref().ok_or_else(|| {
        anyhow!(TokenError::InvalidGrant {
            reason: "subject_token missing `kid` header".to_owned(),
        })
    })?;
    let key = authority::decoding_key_for_kid(kid)
        .map_err(|e| {
            anyhow!(TokenError::InvalidGrant {
                reason: format!("signing key lookup failed: {e}"),
            })
        })?
        .ok_or_else(|| {
            anyhow!(TokenError::InvalidGrant {
                reason: format!("unknown `kid` `{kid}`"),
            })
        })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&global.jwt_issuer]);
    let aud_strs: Vec<&str> = global
        .jwt_audiences
        .iter()
        .map(JwtAudience::as_str)
        .collect();
    validation.set_audience(&aud_strs);
    let data = decode::<JwtClaims>(token, key, &validation).map_err(|e| {
        anyhow!(TokenError::InvalidGrant {
            reason: format!("subject_token rejected: {e}"),
        })
    })?;
    Ok(SubjectIdentity {
        scope: data.claims.scope,
        prior_act: data.claims.act,
    })
}

pub(super) fn jwks_host_allowlist(trusted: &[TrustedIssuer]) -> Vec<String> {
    trusted
        .iter()
        .filter_map(|t| url::Url::parse(&t.jwks_uri).ok())
        .filter_map(|u| u.host_str().map(str::to_owned))
        .collect()
}
