//! RFC 8693 OAuth 2.0 Token Exchange.
//!
//! Trades a `subject_token` issued by a trusted federated identity provider
//! (or by this deployment itself) for a delegated access token bound to the
//! authenticated client. The resulting token carries an `act` claim chain
//! that records every actor who participated in the delegation, oldest
//! delegator innermost. The endpoint also enforces:
//!
//! * `subject_token` issuer is in `profile.security.trusted_issuers` (or is our
//!   own deployment) and signature verifies against that issuer's JWKS;
//! * subject audience matches the trusted-issuer record;
//! * requested `scope` is at most the intersection of subject scope, client
//!   scope, and owner permissions;
//! * `resource` (RFC 8707) is in `allowed_resource_audiences`, otherwise the
//!   call is rejected with `invalid_target`.
//!
//! An ID-JAG subject additionally takes the Enterprise-Managed Authorization
//! path: the token is issued for the employee the ID-JAG names, linked to a
//! local account by `(iss, sub)`, and is pinned to the ID-JAG's `resource`
//! claim when it carries one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_identifiers::ClientId;
use systemprompt_models::Config;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt_with_act};

use super::super::TokenResponse;
use super::RequestOrigin;

mod claims;
mod delegation;
mod id_jag_subject;
mod issue;
mod oidc;
mod subject;

pub use claims::{build_act_chain, intersect_scopes};
#[cfg(feature = "test-api")]
pub use delegation::validate_resource;

#[cfg(not(feature = "test-api"))]
use delegation::validate_resource;
pub use subject::peek_issuer;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::claims::resolve_audience;
    pub use super::id_jag_subject::validate_id_jag_subject;
    pub use super::issue::issue_id_jag;
    pub use super::oidc::validate_oidc_subject;
    pub use super::subject::{SubjectIdentity, jwks_host_allowlist, validate_subject_token};
    pub use super::{ACCESS_TOKEN_TYPE, ID_TOKEN_TYPE, JWT_TOKEN_TYPE, validate_resource};
}

use claims::resolve_audience;
use delegation::{ensure_session, resolve_delegate, resolve_resource};
use id_jag_subject::validate_id_jag_subject;
use issue::issue_id_jag;
use subject::validate_subject_token;
use systemprompt_oauth::services::validation::id_jag::ID_JAG_TOKEN_TYPE;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub const ID_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id_token";
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub const JWT_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:jwt";

#[derive(Debug, Default)]
pub struct TokenExchangeRequest<'a> {
    pub subject_token: &'a str,
    pub subject_token_type: &'a str,
    pub actor_token: Option<&'a str>,
    pub actor_token_type: Option<&'a str>,
    pub requested_token_type: Option<&'a str>,
    pub scope: Option<&'a str>,
    pub audience: Option<&'a str>,
    pub resource: Option<&'a str>,
}

pub async fn handle_token_exchange(
    repo: &OAuthRepository,
    client_id: &ClientId,
    request: TokenExchangeRequest<'_>,
    origin: RequestOrigin<'_>,
    state: &OAuthState,
) -> Result<TokenResponse> {
    let global = Config::get()?;

    if request.requested_token_type == Some(ID_JAG_TOKEN_TYPE) {
        return issue_id_jag(client_id, &request, global).await;
    }

    let subject = if request.subject_token_type == ID_JAG_TOKEN_TYPE {
        validate_id_jag_subject(request.subject_token, client_id, repo, global).await?
    } else {
        validate_subject_token(request.subject_token, request.subject_token_type, global).await?
    };

    let resource = resolve_resource(&subject, request.resource, global)?;
    let (delegate, final_perms) =
        resolve_delegate(repo, state, client_id, &subject, request.scope).await?;

    let audience = resolve_audience(request.audience, global)?;

    let issuer = &global.jwt_issuer;
    let act = build_act_chain(client_id, issuer, subject.prior_act);

    let delegate_uuid = uuid::Uuid::parse_str(delegate.user_id.as_str())
        .map_err(|e| anyhow!("Delegated user has a non-uuid id ({e})"))?;
    let delegated_user = AuthenticatedUser::new(
        delegate_uuid,
        delegate.name,
        delegate.email,
        final_perms.clone(),
    );

    let session_id = ensure_session(state, origin, &delegate.user_id, global).await?;

    let config = JwtConfig {
        permissions: final_perms.clone(),
        audience: audience.clone(),
        expires_in_hours: Some(global.jwt_access_token_expiration / 3600),
        resource,
        plugin_id: None,
    };
    let signing = JwtSigningParams {
        issuer: &global.jwt_issuer,
    };

    let access_token = generate_jwt_with_act(
        &delegated_user,
        config,
        uuid::Uuid::new_v4().to_string(),
        &session_id,
        &signing,
        act,
    )?;

    let scope_string = final_perms
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");

    Ok(TokenResponse {
        access_token,
        token_type: "Bearer".to_owned(),
        expires_in: global.jwt_access_token_expiration,
        refresh_token: None,
        scope: Some(scope_string),
        issued_token_type: Some(ACCESS_TOKEN_TYPE.to_owned()),
    })
}
