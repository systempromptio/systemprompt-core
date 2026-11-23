//! Site-wide auth gate redirecting unauthenticated page requests to login.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::http::{HeaderMap, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use systemprompt_extension::SiteAuthConfig;
use systemprompt_models::auth::{DynRequestAuthenticator, JwtAudience, Permission};
use systemprompt_security::TokenExtractor;

use crate::services::session_cookie::SessionCookie;

const SITE_AUDIENCES: &[JwtAudience] = &[JwtAudience::Web];

const STATIC_ASSET_EXTENSIONS: &[&str] = &[
    ".css", ".js", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".woff", ".woff2", ".ttf",
    ".map", ".webp", ".avif",
];

/// How the site gate verifies a session (the kernel's request authenticator:
/// the web audience, the user row, the session row and revocation; the role is
/// the user row's) and clears a rejected one.
#[derive(Debug, Clone, Copy)]
pub struct SiteAuthSession<'a> {
    pub authenticator: &'a DynRequestAuthenticator,
    pub cookie: SessionCookie,
}

pub async fn site_auth_gate(
    request: Request,
    next: Next,
    config: SiteAuthConfig,
    session: SiteAuthSession<'_>,
) -> Response {
    let path = request.uri().path();

    if path == config.login_path || path == format!("{}/", config.login_path) {
        return next.run(request).await;
    }

    if config
        .public_prefixes
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return next.run(request).await;
    }

    if STATIC_ASSET_EXTENSIONS
        .iter()
        .any(|ext| path.ends_with(ext))
    {
        return next.run(request).await;
    }

    let needs_auth = if config.protected_prefixes.is_empty() {
        true
    } else {
        config
            .protected_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix))
    };

    if !needs_auth {
        return next.run(request).await;
    }

    let outcome = authorize(request.headers(), path, &config, session.authenticator).await;
    match outcome {
        AuthOutcome::Authorized => next.run(request).await,
        outcome => {
            let redirect = login_redirect(config.login_path, request.uri());
            let mut response = Redirect::to(&redirect).into_response();
            if matches!(outcome, AuthOutcome::InvalidToken) {
                response
                    .headers_mut()
                    .insert(header::SET_COOKIE, session.cookie.cleared());
            }
            response
        },
    }
}

enum AuthOutcome {
    Authorized,
    InvalidToken,
    Unauthorized,
}

async fn authorize(
    headers: &HeaderMap,
    path: &str,
    config: &SiteAuthConfig,
    authenticator: &DynRequestAuthenticator,
) -> AuthOutcome {
    if TokenExtractor::browser_only()
        .extract(headers)
        .map_err(|e| tracing::debug!(error = %e, %path, "token extraction failed"))
        .is_err()
    {
        return AuthOutcome::Unauthorized;
    }
    let Ok(required) = config.required_scope.parse::<Permission>().map_err(|e| {
        tracing::warn!(
            error = %e,
            scope = config.required_scope,
            "invalid required_scope config"
        );
    }) else {
        return AuthOutcome::Unauthorized;
    };
    let user = match authenticator.authenticate(headers, SITE_AUDIENCES).await {
        Ok(context) => authenticator.load_user(context.user_id(), "site").await,
        Err(error) => Err(error),
    };
    let user = match user {
        Ok(user) => user,
        Err(error) => {
            tracing::debug!(error = %error, %path, "site session refused; clearing cookie");
            return AuthOutcome::InvalidToken;
        },
    };
    if user
        .roles
        .iter()
        .filter_map(|role| role.parse::<Permission>().ok())
        .any(|role| role.grants(&required))
    {
        AuthOutcome::Authorized
    } else {
        AuthOutcome::Unauthorized
    }
}

#[must_use]
pub fn login_redirect(login_path: &str, uri: &http::Uri) -> String {
    let target = uri
        .path_and_query()
        .map_or_else(|| uri.path(), http::uri::PathAndQuery::as_str);
    format!("{login_path}?redirect={}", urlencoding::encode(target))
}
