//! ID-JAG minting for the EMA issuer role.

use chrono::Utc;
use systemprompt_identifiers::ClientId;

use crate::error::OauthResult as Result;
use crate::services::validation::id_jag::IdJagClaims;

#[derive(Debug)]
pub struct IdJagGrant<'a> {
    pub sub: &'a str,
    pub email: Option<&'a str>,
    pub aud: &'a str,
    pub client_id: &'a str,
    pub scope: Option<&'a str>,
    pub ttl_secs: i64,
    pub issuer: &'a str,
}

/// # Errors
/// Fails if the signing key is unavailable or JWT encoding fails.
pub fn mint_id_jag(grant: &IdJagGrant<'_>) -> Result<String> {
    let now = Utc::now().timestamp();
    let claims = IdJagClaims {
        iss: grant.issuer.to_owned(),
        sub: grant.sub.to_owned(),
        aud: grant.aud.to_owned(),
        client_id: Some(ClientId::new(grant.client_id)),
        azp: None,
        jti: uuid::Uuid::new_v4().to_string(),
        exp: now + grant.ttl_secs,
        iat: now,
        scope: grant.scope.map(ToOwned::to_owned),
        email: grant.email.map(ToOwned::to_owned),
    };
    super::encode_id_jag_with_authority(&claims)
}
