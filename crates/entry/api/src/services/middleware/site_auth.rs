use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use systemprompt_extension::SiteAuthConfig;
use systemprompt_models::auth::Permission;
use systemprompt_security::TokenExtractor;

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

    if let Ok(token) = TokenExtractor::browser_only().extract(request.headers()) {
        let extractor = JwtExtractor::new(&jwt_secret);
        if let Ok(required) = config.required_scope.parse::<Permission>() {
            if let Ok(user_ctx) = extractor.extract_user_context(&token) {
                if user_ctx.role == required {
                    return next.run(request).await;
                }
            }
        }
    }

    let redirect = format!(
        "{}?redirect={}",
        config.login_path,
        urlencoding::encode(path)
    );
    Redirect::to(&redirect).into_response()
}
