//! OIDC `id_token` validation for the EMA issuer path: only a trusted issuer
//! marked `can_issue_id_jag`, matching its `typ_allowlist` and `audience`, may
//! seed an ID-JAG.

use anyhow::{Result, anyhow};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use systemprompt_models::Config;
use systemprompt_security::keys::JwksClient;

use super::super::super::TokenError;
use super::subject::{jwks_host_allowlist, peek_issuer};

pub(super) struct OidcSubject {
    pub(super) sub: String,
    pub(super) email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OidcIdTokenClaims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
}

pub(super) async fn validate_oidc_subject(token: &str, global: &Config) -> Result<OidcSubject> {
    let header = decode_header(token).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("malformed id_token header: {e}"),
        })
    })?;

    let declared_iss = peek_issuer(token)?;
    let trusted = global
        .trusted_issuers
        .iter()
        .find(|t| t.issuer == declared_iss && t.can_issue_id_jag)
        .ok_or_else(|| {
            anyhow!(TokenError::InvalidRequest {
                field: "subject_token".to_owned(),
                message: format!("issuer '{declared_iss}' is not a trusted ID-JAG issuer"),
            })
        })?;

    if !trusted.typ_allowlist.is_empty()
        && !header
            .typ
            .as_deref()
            .is_some_and(|t| trusted.typ_allowlist.iter().any(|a| a == t))
    {
        return Err(anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("id_token typ {:?} not in issuer typ_allowlist", header.typ),
        }));
    }

    if header.alg != Algorithm::RS256 {
        return Err(anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: "id_token must be RS256-signed".to_owned(),
        }));
    }
    let kid = header.kid.ok_or_else(|| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: "id_token must carry a kid header".to_owned(),
        })
    })?;

    let jwk = JwksClient::new(jwks_host_allowlist(&global.trusted_issuers))
        .fetch_at(&trusted.issuer, &trusted.jwks_uri, &kid)
        .await
        .map_err(|e| {
            anyhow!(TokenError::InvalidRequest {
                field: "subject_token".to_owned(),
                message: format!("JWKS resolution failed: {e}"),
            })
        })?;
    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("invalid RSA components in JWK: {e}"),
        })
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&trusted.issuer]);
    validation.set_audience(&[&trusted.audience]);

    let data = decode::<OidcIdTokenClaims>(token, &key, &validation).map_err(|e| {
        anyhow!(TokenError::InvalidRequest {
            field: "subject_token".to_owned(),
            message: format!("id_token signature/claims rejected: {e}"),
        })
    })?;

    Ok(OidcSubject {
        sub: data.claims.sub,
        email: data.claims.email,
    })
}
