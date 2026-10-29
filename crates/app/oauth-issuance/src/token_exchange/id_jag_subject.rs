//! EMA resource-server consume path.
//!
//! Validates an ID-JAG presented as a token-exchange subject — `typ`,
//! audience, client binding, and single-use `jti` replay. Signature is checked
//! against the local authority for a self-issued ID-JAG, otherwise the trusted
//! issuer's JWKS.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use chrono::{TimeZone, Utc};
use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use systemprompt_identifiers::{AccessTokenId, ClientId};
use systemprompt_manifest::Config;
use systemprompt_models::auth::Permission;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::EnterprisePrincipal;
use systemprompt_oauth::services::validation::id_jag::{
    ClaimPolicy, DEFAULT_LEEWAY_SECS, IdJagClaims, validate_claims, validate_typ,
};
use systemprompt_security::jwt::reject_critical_header;
use systemprompt_security::keys::SigningKeys;

use super::subject::{JwksCache, REQUIRED_CLAIMS, SubjectIdentity, peek_issuer};
use crate::{IssuanceError, IssuanceResult};

#[derive(Debug, Clone, Copy)]
pub struct IdJagContext<'a> {
    pub repo: &'a OAuthRepository,
    pub keys: &'a SigningKeys,
    pub global: &'a Config,
    pub jwks: &'a JwksCache,
}

pub async fn validate_id_jag_subject(
    token: &str,
    authenticated_client: &ClientId,
    context: IdJagContext<'_>,
) -> IssuanceResult<SubjectIdentity> {
    let IdJagContext {
        repo,
        keys,
        global,
        jwks,
    } = context;
    let (claims, allowed_client_ids) = verify_id_jag_signature(keys, token, global, jwks).await?;

    let policy = ClaimPolicy {
        expected_audience: global.jwt_issuer.as_str(),
        authenticated_client: authenticated_client.as_str(),
        allowed_client_ids: &allowed_client_ids,
        now: Utc::now().timestamp(),
        leeway: DEFAULT_LEEWAY_SECS,
    };
    validate_claims(&claims, &policy).map_err(IssuanceError::IdJagRejected)?;

    let expires_at =
        Utc.timestamp_opt(claims.exp, 0)
            .single()
            .ok_or_else(|| IssuanceError::InvalidGrant {
                reason: "ID-JAG exp is out of range".to_owned(),
            })?;
    let jti =
        AccessTokenId::try_new(claims.jti.as_str()).map_err(|e| IssuanceError::RejectedGrant {
            reason: "ID-JAG jti is invalid",
            source: e.into(),
        })?;
    let first_use = repo
        .consume_id_jag_jti(&jti, expires_at)
        .await
        .map_err(|e| IssuanceError::server("ID-JAG replay store unavailable", e))?;
    if !first_use {
        return Err(IssuanceError::InvalidGrant {
            reason: "ID-JAG has already been used (replay)".to_owned(),
        });
    }

    let scope = claims
        .scope
        .as_deref()
        .map(|s| {
            s.split_whitespace()
                .filter_map(|p| Permission::from_str(p).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(SubjectIdentity {
        scope,
        prior_act: None,
        interaction: None,
        principal: Some(EnterprisePrincipal {
            issuer: claims.iss,
            sub: claims.sub,
            email: claims.email,
            email_verified: claims.email_verified,
        }),
        bound_resource: claims.resource,
    })
}

async fn verify_id_jag_signature(
    keys: &SigningKeys,
    token: &str,
    global: &Config,
    jwks: &JwksCache,
) -> IssuanceResult<(IdJagClaims, Vec<ClientId>)> {
    let header = decode_header(token)
        .map_err(|e| IssuanceError::rejected_grant("ID-JAG header decode failed", e))?;
    reject_critical_header(&header)
        .map_err(|e| IssuanceError::rejected_grant("ID-JAG crit header unsupported", e))?;
    validate_typ(header.typ.as_deref()).map_err(IssuanceError::IdJagRejected)?;
    let kid = header.kid.ok_or_else(|| IssuanceError::InvalidGrant {
        reason: "ID-JAG missing `kid` header".to_owned(),
    })?;

    let declared_iss = peek_issuer(token)?;

    let validation = |alg: Algorithm| {
        let mut validation = Validation::new(alg);
        validation.set_issuer(&[declared_iss.as_str()]);
        validation.set_audience(&[global.jwt_issuer.as_str()]);
        validation.set_required_spec_claims(&REQUIRED_CLAIMS);
        validation
    };

    if declared_iss == global.jwt_issuer {
        if header.alg != Algorithm::RS256 {
            return Err(IssuanceError::InvalidGrant {
                reason: "ID-JAG must be RS256-signed".to_owned(),
            });
        }
        let key = keys
            .decoding_key_for_kid(&kid)
            .ok_or_else(|| IssuanceError::InvalidGrant {
                reason: format!("unknown `kid` `{kid}`"),
            })?;
        let data = decode::<IdJagClaims>(token, key, &validation(Algorithm::RS256))
            .map_err(|e| IssuanceError::rejected_grant("ID-JAG rejected", e))?;
        return Ok((data.claims, Vec::new()));
    }

    let trusted = global
        .trusted_issuers
        .iter()
        .find(|t| t.issuer == declared_iss)
        .ok_or_else(|| IssuanceError::InvalidGrant {
            reason: format!("ID-JAG issuer '{declared_iss}' is not trusted"),
        })?;
    let jwk = jwks
        .client_for(trusted)
        .fetch_at(&trusted.issuer, &trusted.jwks_uri, &kid)
        .await
        .map_err(|e| IssuanceError::rejected_grant("ID-JAG JWKS resolution failed", e))?;
    if header.alg != jwk.algorithm() {
        return Err(IssuanceError::InvalidGrant {
            reason: format!(
                "ID-JAG algorithm {:?} does not match the issuer key",
                header.alg
            ),
        });
    }
    let key = jwk
        .decoding_key()
        .map_err(|e| IssuanceError::rejected_grant("invalid key components in JWK", e))?;
    let data = decode::<IdJagClaims>(token, &key, &validation(jwk.algorithm()))
        .map_err(|e| IssuanceError::rejected_grant("ID-JAG rejected", e))?;
    Ok((data.claims, trusted.allowed_client_ids.clone()))
}
