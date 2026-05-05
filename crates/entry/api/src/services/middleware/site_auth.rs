use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use systemprompt_extension::SiteAuthConfig;
use systemprompt_models::auth::Permission;
use systemprompt_security::TokenExtractor;
use tracing;

use super::jwt::JwtExtractor;

const STATIC_ASSET_EXTENSIONS: &[&str] = &[
    ".css", ".js", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".woff", ".woff2", ".ttf",
    ".map", ".webp", ".avif",
];

pub async fn site_auth_gate(
    request: Request,
    next: Next,
    config: SiteAuthConfig,
    jwt_secret: String,
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

    let auth_result = TokenExtractor::browser_only()
        .extract(request.headers())
        .map_err(|e| tracing::debug!(error = %e, %path, "token extraction failed"))
        .ok()
        .and_then(|token| {
            let required = config
                .required_scope
                .parse::<Permission>()
                .map_err(|e| {
                    tracing::warn!(
                        error = %e,
                        scope = config.required_scope,
                        "invalid required_scope config"
                    );
                })
                .ok()?;
            let extractor = JwtExtractor::new(&jwt_secret);
            let user_ctx = extractor
                .extract_user_context(&token)
                .map_err(|e| tracing::debug!(error = %e, %path, "jwt validation failed"))
                .ok()?;
            (user_ctx.role == required).then_some(())
        });

    if auth_result.is_some() {
        return next.run(request).await;
    }

    let redirect = format!(
        "{}?redirect={}",
        config.login_path,
        urlencoding::encode(path)
    );
    Redirect::to(&redirect).into_response()
}
