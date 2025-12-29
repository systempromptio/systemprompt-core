use axum::extract::Request;
use axum::http::HeaderMap;
use axum::middleware;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_models::modules::ApiPaths;

use super::jwt::extract_token_from_headers;

#[derive(Debug, Clone)]
pub struct ApiAuthMiddlewareConfig {
    pub public_paths: Vec<&'static str>,
}

impl Default for ApiAuthMiddlewareConfig {
    fn default() -> Self {
        Self {
            public_paths: vec![
                ApiPaths::OAUTH_SESSION,
                ApiPaths::OAUTH_REGISTER,
                ApiPaths::OAUTH_AUTHORIZE,
                ApiPaths::OAUTH_TOKEN,
                ApiPaths::OAUTH_CALLBACK,
                ApiPaths::OAUTH_CONSENT,
                ApiPaths::OAUTH_WEBAUTHN_COMPLETE,
                ApiPaths::WELLKNOWN_BASE,
                ApiPaths::STREAM_BASE,
                ApiPaths::CONTEXTS_WEBHOOK,
                ApiPaths::DISCOVERY,
            ],
        }
    }
}

impl ApiAuthMiddlewareConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_public_path(&self, path: &str) -> bool {
        if !path.starts_with(ApiPaths::API_BASE) && !path.starts_with(ApiPaths::WELLKNOWN_BASE) {
            return true;
        }

        self.public_paths.iter().any(|p| path.starts_with(p))
            || path.starts_with(ApiPaths::WELLKNOWN_BASE)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AuthMiddleware;

impl AuthMiddleware {
    pub fn apply_auth_layer(router: axum::Router) -> axum::Router {
        router.layer(middleware::from_fn(move |req, next| {
            let config = ApiAuthMiddlewareConfig::default();
            async move { auth_middleware(config, req, next).await }
        }))
    }
}

pub async fn auth_middleware(
    config: ApiAuthMiddlewareConfig,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    if config.is_public_path(path) {
        return next.run(req).await;
    }

    if let Some(user) = extract_optional_user(req.headers()) {
        req.extensions_mut().insert(user);
    }

    next.run(req).await
}

fn extract_optional_user(headers: &HeaderMap) -> Option<systemprompt_models::AuthenticatedUser> {
    use systemprompt_core_oauth::validate_jwt_token;
    use systemprompt_models::auth::UserType;
    use systemprompt_models::SecretsBootstrap;
    use uuid::Uuid;

    let token = extract_token_from_headers(headers)?;

    if token.trim().is_empty() {
        return None;
    }

    let jwt_secret = SecretsBootstrap::jwt_secret().ok()?;
    let config = systemprompt_models::Config::get().ok()?;
    let claims = match validate_jwt_token(
        &token,
        jwt_secret,
        &config.jwt_issuer,
        &config.jwt_audiences,
    ) {
        Ok(claims) => claims,
        Err(e) => {
            tracing::warn!(error = %e, "JWT validation failed");
            return None;
        },
    };

    let user_id = Uuid::parse_str(&claims.sub).ok()?;

    let email = if claims.email.is_empty() || claims.user_type == UserType::Anon {
        None
    } else {
        Some(claims.email.clone())
    };

    let permissions = claims.scope;

    Some(systemprompt_models::AuthenticatedUser::new(
        user_id,
        claims.username,
        email,
        permissions,
    ))
}
