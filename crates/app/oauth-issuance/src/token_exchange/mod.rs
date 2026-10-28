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

use std::sync::Arc;
use systemprompt_identifiers::{AccessTokenId, ClientId};
use systemprompt_manifest::Config;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::{
    DelegatedJwtParams, JwtConfig, JwtSigningParams, generate_jwt_with_act,
};

use crate::{IssuanceError, IssuanceResult, RequestOrigin, TokenResponse};

pub mod claims;
pub mod delegation;
pub mod id_jag_subject;
pub mod issue;
pub mod oidc;
pub mod subject;

pub use claims::{build_act_chain, intersect_scopes};
pub use delegation::validate_resource;
pub use subject::{JwksCache, peek_issuer};

use claims::resolve_audience;
use delegation::{ensure_session, resolve_delegate, resolve_resource};
use id_jag_subject::{IdJagContext, validate_id_jag_subject};
use issue::issue_id_jag;
use subject::{SubjectIdentity, validate_subject_token};
use systemprompt_oauth::services::validation::id_jag::ID_JAG_TOKEN_TYPE;

pub const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
pub const ID_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id_token";
pub const JWT_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:jwt";

#[derive(Debug, Clone, Copy)]
pub struct ExchangeContext<'a> {
    pub jwks: &'a JwksCache,
    pub config: &'a Arc<Config>,
}

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

async fn resolve_subject(
    client_id: &ClientId,
    request: &TokenExchangeRequest<'_>,
    state: &OAuthState,
    global: &Config,
    jwks: &JwksCache,
) -> IssuanceResult<SubjectIdentity> {
    if request.subject_token_type == ID_JAG_TOKEN_TYPE {
        validate_id_jag_subject(
            request.subject_token,
            client_id,
            IdJagContext {
                repo: state.oauth_repository(),
                keys: state.signing_keys(),
                global,
                jwks,
            },
        )
        .await
    } else {
        validate_subject_token(
            state.signing_keys(),
            request.subject_token,
            request.subject_token_type,
            global,
            jwks,
        )
        .await
    }
}

pub async fn handle_token_exchange(
    client_id: &ClientId,
    request: TokenExchangeRequest<'_>,
    origin: RequestOrigin<'_>,
    state: &OAuthState,
    env: ExchangeContext<'_>,
) -> IssuanceResult<TokenResponse> {
    let ExchangeContext {
        jwks,
        config: global,
    } = env;
    let repo = state.oauth_repository();

    if request.requested_token_type == Some(ID_JAG_TOKEN_TYPE) {
        return issue_id_jag(state.signing_keys(), client_id, &request, global, jwks).await;
    }

    let subject = resolve_subject(client_id, &request, state, global, jwks).await?;

    let resource = resolve_resource(&subject, request.resource, global)?;
    let (delegate, final_perms) =
        resolve_delegate(repo, state, client_id, &subject, request.scope).await?;

    let audience = resolve_audience(request.audience, global)?;

    let issuer = &global.jwt_issuer;
    let act = build_act_chain(client_id, issuer, subject.prior_act);

    let delegated_user = AuthenticatedUser::new(
        delegate.user_id.clone(),
        delegate.name,
        delegate.email,
        final_perms.clone(),
    );

    let session_id = ensure_session(state, origin, &delegate.user_id, global).await?;

    let config = JwtConfig {
        permissions: final_perms.clone(),
        audience: audience.clone(),
        expires_in: chrono::Duration::seconds(global.jwt_access_token_expiration),
        resource,
        plugin_id: None,
        client_id: Some(client_id.clone()),
        purpose: None,
        interaction: subject.interaction,
    };
    let signing = JwtSigningParams {
        issuer: &global.jwt_issuer,
        keys: state.signing_keys(),
    };

    let access_token = generate_jwt_with_act(DelegatedJwtParams {
        user: &delegated_user,
        config,
        jti: AccessTokenId::generate(),
        session_id: &session_id,
        signing: &signing,
        act,
    })
    .map_err(|e| IssuanceError::server("Access token signing failed", e))?;

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
