//! Site-wide auth gate redirecting unauthenticated page requests to login.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use systemprompt_extension::SiteAuthConfig;
use systemprompt_models::auth::Permission;
use systemprompt_security::{TokenExtractor, extract_user_context};

// Why: Purges a token minted under a previous `security.issuer` instead of
// bouncing the browser between login and the protected page forever;
// `Secure` is omitted because a `Secure` deletion is discarded on
// plain-HTTP local deployments.
const CLEAR_ACCESS_TOKEN_COOKIE: &str =
    "access_token=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict";

const STATIC_ASSET_EXTENSIONS: &[&str] = &[
    ".css", ".js", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".woff", ".woff2", ".ttf",
    ".map", ".webp", ".avif",
];

pub async fn site_auth_gate(request: Request, next: Next, config: SiteAuthConfig) -> Response {
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

    match authorize(&request, &config) {
        AuthOutcome::Authorized => next.run(request).await,
        outcome => {
            let redirect = login_redirect(config.login_path, request.uri());
            let mut response = Redirect::to(&redirect).into_response();
            if matches!(outcome, AuthOutcome::InvalidToken)
                && let Ok(cookie) = HeaderValue::from_str(CLEAR_ACCESS_TOKEN_COOKIE)
            {
                response.headers_mut().insert(header::SET_COOKIE, cookie);
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

fn authorize(request: &Request, config: &SiteAuthConfig) -> AuthOutcome {
    let path = request.uri().path();
    let Ok(token) = TokenExtractor::browser_only()
        .extract(request.headers())
        .map_err(|e| tracing::debug!(error = %e, %path, "token extraction failed"))
    else {
        return AuthOutcome::Unauthorized;
    };
    let Ok(required) = config.required_scope.parse::<Permission>().map_err(|e| {
        tracing::warn!(
            error = %e,
            scope = config.required_scope,
            "invalid required_scope config"
        );
    }) else {
        return AuthOutcome::Unauthorized;
    };
    let Ok(user_ctx) = extract_user_context(&token)
        .map_err(|e| tracing::debug!(error = %e, %path, "jwt validation failed; clearing cookie"))
    else {
        return AuthOutcome::InvalidToken;
    };
    if user_ctx.role == required || user_ctx.role.implies(&required) {
        AuthOutcome::Authorized
    } else {
        AuthOutcome::Unauthorized
    }
}

/// The bridge device-link carries its loopback callback in `?redirect=...`, so
/// dropping the query strands the post-login bounce on a page whose extractor
/// then 400s.
#[must_use]
pub fn login_redirect(login_path: &str, uri: &http::Uri) -> String {
    let target = uri
        .path_and_query()
        .map_or_else(|| uri.path(), http::uri::PathAndQuery::as_str);
    format!("{login_path}?redirect={}", urlencoding::encode(target))
}
