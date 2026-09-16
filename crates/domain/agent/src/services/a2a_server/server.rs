//! The per-agent A2A HTTP server.
//!
//! [`Server`] loads an agent's configuration, wires OAuth state and the AI
//! provider, and builds the axum [`Router`] exposing the agent card and the A2A
//! request endpoint, then runs the listener.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, post};
use axum::{Router, middleware};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::{AgentConfig, AiProvider};
use tokio::sync::{RwLock, Semaphore};
use tower_http::cors::{AllowOrigin, CorsLayer};

use super::active_tasks::ActiveTasks;
use super::auth::{AgentOAuthConfig, AgentOAuthState, agent_oauth_middleware_wrapper};
use super::handlers::{AgentHandlerState, handle_agent_card, handle_agent_request};
use crate::state::AgentState;

// Why: A2A file parts travel inline as base64, so the JSON-RPC body cap is
// deliberately wider than the API's default request limit.
pub const A2A_MAX_REQUEST_BODY_BYTES: usize = 8 * 1024 * 1024;

fn cors_layer(origins: &[String]) -> Result<CorsLayer, crate::error::AgentError> {
    let mut allowed = Vec::new();
    for origin in origins {
        let trimmed = origin.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value = trimmed.parse::<HeaderValue>().map_err(|e| {
            crate::error::AgentError::Config(format!(
                "invalid cors_allowed_origins entry {origin:?}: {e}"
            ))
        })?;
        allowed.push(value);
    }
    if allowed.is_empty() {
        return Err(crate::error::AgentError::Config(
            "cors_allowed_origins must contain at least one valid origin".to_owned(),
        ));
    }
    Ok(CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed))
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            http::header::AUTHORIZATION,
            http::header::CONTENT_TYPE,
            http::header::ACCEPT,
        ]))
}

pub struct Server {
    config: Arc<RwLock<AgentConfig>>,
    oauth_state: Arc<AgentOAuthState>,
    agent_state: Arc<AgentState>,
    ai_service: Arc<dyn AiProvider>,
    stream_semaphore: Arc<Semaphore>,
    active_tasks: ActiveTasks,
    cors: CorsLayer,
    port: u16,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("config", &"Arc<RwLock<AgentConfig>>")
            .field("oauth_state", &"Arc<AgentOAuthState>")
            .field("agent_state", &"Arc<AgentState>")
            .field("ai_service", &"<Arc<dyn AiProvider>>")
            .field(
                "stream_semaphore",
                &self.stream_semaphore.available_permits(),
            )
            .field("active_tasks", &self.active_tasks)
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl Server {
    pub async fn new(
        db_pool: DbPool,
        agent_state: Arc<AgentState>,
        ai_service: Arc<dyn AiProvider>,
        agent_name: Option<String>,
        port: u16,
    ) -> Result<Self, crate::error::AgentError> {
        use crate::services::registry::AgentRegistry;

        let mut config = if let Some(name) = agent_name {
            let registry = AgentRegistry::new()
                .map_err(|e| crate::error::AgentError::Server(e.to_string()))?;
            registry
                .get_agent(&name)
                .await
                .map_err(|e| crate::error::AgentError::Server(e.to_string()))?
        } else {
            return Err(crate::error::AgentError::Validation(
                "Agent name is required".to_owned(),
            ));
        };

        config.extract_oauth_scopes_from_card();

        let oauth_config = AgentOAuthConfig::default();
        let global_config = agent_state.config();
        let mut oauth_state = AgentOAuthState::new(
            Arc::clone(&db_pool),
            oauth_config,
            global_config.jwt_issuer.clone(),
            global_config.jwt_audiences.clone(),
        );

        oauth_state = oauth_state.with_jwt_provider(Arc::clone(agent_state.jwt_provider()));
        let cors = cors_layer(&global_config.cors_allowed_origins)?;
        let stream_semaphore = Arc::new(Semaphore::new(global_config.max_concurrent_streams));

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            oauth_state: Arc::new(oauth_state),
            agent_state,
            ai_service,
            stream_semaphore,
            active_tasks: ActiveTasks::default(),
            cors,
            port,
        })
    }

    pub fn create_router(&self) -> Router {
        let state = Arc::new(AgentHandlerState {
            config: Arc::clone(&self.config),
            oauth_state: Arc::clone(&self.oauth_state),
            agent_state: Arc::clone(&self.agent_state),
            ai_service: Arc::clone(&self.ai_service),
            stream_semaphore: Arc::clone(&self.stream_semaphore),
            active_tasks: self.active_tasks.clone(),
        });

        let post_router = Router::new()
            .route("/", post(handle_agent_request))
            .layer(DefaultBodyLimit::max(A2A_MAX_REQUEST_BODY_BYTES))
            .with_state(Arc::clone(&state))
            .layer(middleware::from_fn_with_state(
                Arc::clone(&state),
                agent_oauth_middleware_wrapper,
            ));

        let get_router = Router::new()
            .route(ApiPaths::WELLKNOWN_AGENT_CARD, get(handle_agent_card))
            .route(ApiPaths::A2A_CARD, get(handle_agent_card))
            .with_state(state);

        Router::new()
            .merge(post_router)
            .merge(get_router)
            .layer(self.cors.clone())
    }

    // Why: the listener stops accepting on `shutdown`, then the server waits
    // for every stream worker it spawned so an in-flight task is persisted
    // rather than torn down mid-write.
    pub async fn run<F>(self, shutdown: F) -> Result<(), crate::error::AgentError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let app = self.create_router();
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        tracing::info!(
            addr = %addr,
            max_concurrent_streams = self.stream_semaphore.available_permits(),
            "A2A server listening"
        );

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|e| crate::error::AgentError::Server(e.to_string()))?;

        self.active_tasks.tracker().close();
        self.active_tasks.tracker().wait().await;
        Ok(())
    }
}
