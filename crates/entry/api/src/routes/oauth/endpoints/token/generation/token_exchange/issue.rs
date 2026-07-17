//! EMA issuer path: on `requested_token_type = id-jag`, mint a short-lived
//! ID-JAG from a validated upstream OIDC `id_token`, bound to the authenticated
//! token-exchange client. Mirror of [`super::id_jag_subject`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_identifiers::ClientId;
use systemprompt_models::Config;
use systemprompt_oauth::services::generation::{IdJagGrant, mint_id_jag};
use systemprompt_oauth::services::validation::id_jag::ID_JAG_TOKEN_TYPE;

use super::super::super::{TokenError, TokenResponse};
use super::oidc::validate_oidc_subject;
use super::{ID_TOKEN_TYPE, JWT_TOKEN_TYPE, TokenExchangeRequest};

pub(super) async fn issue_id_jag(
    client_id: &ClientId,
    request: &TokenExchangeRequest<'_>,
    global: &Config,
) -> Result<TokenResponse> {
    if !matches!(request.subject_token_type, ID_TOKEN_TYPE | JWT_TOKEN_TYPE) {
        return Err(anyhow!(TokenError::InvalidRequest {
            field: "subject_token_type".to_owned(),
            message: format!(
                "ID-JAG issuance requires an id_token/jwt subject, got '{}'",
                request.subject_token_type
            ),
        }));
    }

    let subject = validate_oidc_subject(request.subject_token, global).await?;

    let aud = match request.audience {
        None => global.jwt_issuer.as_str(),
        Some(a) if a == global.jwt_issuer => a,
        Some(a) if global.allowed_resource_audiences.iter().any(|r| r == a) => a,
        Some(a) => {
            return Err(anyhow!(TokenError::InvalidTarget {
                message: format!(
                    "audience '{a}' is neither this issuer nor an allowed resource audience"
                ),
            }));
        },
    };

    let id_jag = mint_id_jag(&IdJagGrant {
        sub: &subject.sub,
        email: subject.email.as_deref(),
        aud,
        client_id,
        scope: request.scope,
        ttl_secs: global.id_jag_ttl_secs,
        issuer: &global.jwt_issuer,
    })
    .map_err(|e| {
        anyhow!(TokenError::ServerError {
            message: format!("ID-JAG minting failed: {e}"),
        })
    })?;

    Ok(TokenResponse {
        access_token: id_jag,
        token_type: "N_A".to_owned(),
        expires_in: global.id_jag_ttl_secs,
        refresh_token: None,
        scope: request.scope.map(ToOwned::to_owned),
        issued_token_type: Some(ID_JAG_TOKEN_TYPE.to_owned()),
    })
}
