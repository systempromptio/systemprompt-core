use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;
use systemprompt_core_analytics::AnalyticsService;
use systemprompt_core_oauth::services::SessionCreationService;
use systemprompt_core_users::{UserProviderImpl, UserService};
use systemprompt_identifiers::{AgentName, ClientId, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;
use uuid::Uuid;

use super::jwt::{extract_token_from_headers, JwtExtractor};

#[derive(Clone, Debug)]
pub struct SessionMiddleware {
    jwt_extractor: Arc<JwtExtractor>,
    analytics_service: Arc<AnalyticsService>,
    session_creation_service: Arc<SessionCreationService>,
}

impl SessionMiddleware {
    pub fn new(ctx: &AppContext) -> anyhow::Result<Self> {
        let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?;
        let jwt_extractor = Arc::new(JwtExtractor::new(jwt_secret));
        let user_service = UserService::new(ctx.db_pool())?;
        let session_creation_service = Arc::new(SessionCreationService::new(
            ctx.analytics_service().clone(),
            Arc::new(UserProviderImpl::new(user_service)),
        ));

        Ok(Self {
            jwt_extractor,
            analytics_service: ctx.analytics_service().clone(),
            session_creation_service,
        })
    }

    pub async fn handle(&self, mut request: Request, next: Next) -> Result<Response, StatusCode> {
        let headers = request.headers();
        let uri = request.uri().clone();
        let method = request.method().clone();

        let should_skip = Self::should_skip_session_tracking(uri.path());

        let trace_id = headers
            .get("x-trace-id")
            .and_then(|h| h.to_str().ok())
            .map_or_else(
                || TraceId::new(Uuid::new_v4().to_string()),
                |s| TraceId::new(s.to_string()),
            );

        let (req_ctx, jwt_cookie) = if should_skip {
            let ctx = RequestContext::new(
                SessionId::new(format!("untracked_{}", Uuid::new_v4())),
                trace_id,
                ContextId::new(String::new()),
                AgentName::system(),
            )
            .with_user_id(UserId::new("anonymous".to_string()))
            .with_user_type(UserType::Anon)
            .with_tracked(false);
            (ctx, None)
        } else {
            let analytics = self
                .analytics_service
                .extract_analytics(headers, Some(&uri));
            let is_bot = AnalyticsService::is_bot(&analytics);

            if is_bot {
                let ctx = RequestContext::new(
                    SessionId::new(format!("bot_{}", Uuid::new_v4())),
                    trace_id,
                    ContextId::new(String::new()),
                    AgentName::system(),
                )
                .with_user_id(UserId::new("bot".to_string()))
                .with_user_type(UserType::Anon)
                .with_tracked(false);
                (ctx, None)
            } else {
                let token_result = extract_token_from_headers(headers);

                let (session_id, user_id, jwt_token, jwt_cookie) = if let Some(token) = token_result
                {
                    if let Ok(jwt_context) = self.jwt_extractor.extract_user_context(&token) {
                        (jwt_context.session_id, jwt_context.user_id, token, None)
                    } else {
                        let (sid, uid, token, is_new) =
                            self.create_new_session(headers, &uri, &method).await?;
                        let jwt_cookie = if is_new { Some(token.clone()) } else { None };
                        (sid, uid, token, jwt_cookie)
                    }
                } else {
                    let (sid, uid, token, is_new) =
                        self.create_new_session(headers, &uri, &method).await?;
                    let jwt_cookie = if is_new { Some(token.clone()) } else { None };
                    (sid, uid, token, jwt_cookie)
                };

                let ctx = RequestContext::new(
                    session_id,
                    trace_id,
                    ContextId::new(String::new()),
                    AgentName::system(),
                )
                .with_user_id(user_id)
                .with_auth_token(jwt_token)
                .with_user_type(UserType::Anon)
                .with_tracked(true);
                (ctx, jwt_cookie)
            }
        };

        request.extensions_mut().insert(req_ctx);

        let mut response = next.run(request).await;

        if let Some(token) = jwt_cookie {
            let cookie =
                format!("access_token={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800");
            if let Ok(cookie_value) = cookie.parse() {
                response
                    .headers_mut()
                    .insert(header::SET_COOKIE, cookie_value);
            }
        }

        Ok(response)
    }

    async fn create_new_session(
        &self,
        headers: &http::HeaderMap,
        uri: &http::Uri,
        _method: &http::Method,
    ) -> Result<(SessionId, UserId, String, bool), StatusCode> {
        let client_id = ClientId::new("sp_web".to_string());

        match self
            .session_creation_service
            .create_anonymous_session(
                headers,
                Some(uri),
                &client_id,
                systemprompt_models::SecretsBootstrap::jwt_secret().map_err(|e| {
                    tracing::error!(error = %e, "Failed to get JWT secret during session creation");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?,
            )
            .await
        {
            Ok(session_info) => Ok((
                session_info.session_id,
                session_info.user_id,
                session_info.jwt_token,
                session_info.is_new,
            )),
            Err(e) => {
                tracing::error!(error = %e, "Failed to create anonymous session");
                Err(StatusCode::SERVICE_UNAVAILABLE)
            },
        }
    }

    fn should_skip_session_tracking(path: &str) -> bool {
        if path.starts_with(ApiPaths::API_BASE) {
            return true;
        }

        if path.starts_with(ApiPaths::NEXT_BASE) {
            return true;
        }

        if path.starts_with(ApiPaths::STATIC_BASE)
            || path.starts_with(ApiPaths::ASSETS_BASE)
            || path.starts_with(ApiPaths::IMAGES_BASE)
        {
            return true;
        }

        if path == "/health" || path == "/ready" || path == "/healthz" {
            return true;
        }

        if path == "/favicon.ico"
            || path == "/robots.txt"
            || path == "/sitemap.xml"
            || path == "/manifest.json"
        {
            return true;
        }

        if let Some(last_segment) = path.rsplit('/').next() {
            if last_segment.contains('.') {
                let extension = last_segment.rsplit('.').next().unwrap_or("");
                match extension {
                    "html" | "htm" => {},
                    _ => return true,
                }
            }
        }

        false
    }
}
