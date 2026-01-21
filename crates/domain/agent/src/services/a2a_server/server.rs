use axum::routing::{get, post};
use axum::{middleware, Router};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::{AgentConfig, AiProvider};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use super::auth::{agent_oauth_middleware_wrapper, AgentOAuthConfig, AgentOAuthState};
use super::handlers::{handle_agent_card, handle_agent_request, AgentHandlerState};
use crate::state::AgentState;

pub struct Server {
    db_pool: DbPool,
    config: Arc<RwLock<AgentConfig>>,
    oauth_state: Arc<AgentOAuthState>,
    agent_state: Arc<AgentState>,
    ai_service: Arc<dyn AiProvider>,
    port: u16,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("pool", &"SqlitePool")
            .field("config", &"Arc<RwLock<AgentConfig>>")
            .field("oauth_state", &"Arc<AgentOAuthState>")
            .field("agent_state", &"Arc<AgentState>")
            .field("ai_service", &"<Arc<dyn AiProvider>>")
            .field("port", &self.port)
            .finish()
    }
}

impl Server {
    pub async fn new(
        db_pool: DbPool,
        agent_state: Arc<AgentState>,
        ai_service: Arc<dyn AiProvider>,
        agent_name: Option<String>,
        port: u16,
    ) -> anyhow::Result<Self> {
        use crate::services::registry::AgentRegistry;

        let mut config = if let Some(name) = agent_name {
            let registry = AgentRegistry::new().await?;
            registry.get_agent(&name).await?
        } else {
            return Err(anyhow::anyhow!("Agent name is required"));
        };

        config.extract_oauth_scopes_from_card();

        let oauth_config = AgentOAuthConfig::default();
        let jwt_secret = systemprompt_models::SecretsBootstrap::jwt_secret()?.to_string();
        let global_config = systemprompt_models::Config::get()?;
        let mut oauth_state = AgentOAuthState::new(
            db_pool.clone(),
            oauth_config,
            jwt_secret,
            global_config.jwt_issuer.clone(),
            global_config.jwt_audiences.clone(),
        )
        .await?;

        oauth_state = oauth_state.with_jwt_provider(Arc::clone(agent_state.jwt_provider()));
        if let Some(user_provider) = agent_state.user_provider().cloned() {
            oauth_state = oauth_state.with_user_provider(user_provider);
        }

        Ok(Self {
            db_pool,
            config: Arc::new(RwLock::new(config)),
            oauth_state: Arc::new(oauth_state),
            agent_state,
            ai_service,
            port,
        })
    }

    pub async fn reload_config(&self) -> anyhow::Result<()> {
        use crate::services::registry::AgentRegistry;

        let agent_name = {
            let config = self.config.read().await;
            config.name.clone()
        };

        let registry = AgentRegistry::new().await?;
        let mut new_config = registry.get_agent(&agent_name).await?;
        new_config.extract_oauth_scopes_from_card();
        *self.config.write().await = new_config;

        tracing::info!(agent_name = %agent_name, "Configuration reloaded");
        Ok(())
    }

    pub fn create_router(&self) -> Router {
        let state = Arc::new(AgentHandlerState {
            db_pool: self.db_pool.clone(),
            config: Arc::clone(&self.config),
            oauth_state: Arc::clone(&self.oauth_state),
            agent_state: Arc::clone(&self.agent_state),
            ai_service: Arc::clone(&self.ai_service),
        });

        let post_router = Router::new()
            .route("/", post(handle_agent_request))
            .with_state(state.clone())
            .layer(middleware::from_fn_with_state(
                state.clone(),
                agent_oauth_middleware_wrapper,
            ));

        let get_router = Router::new()
            .route(ApiPaths::WELLKNOWN_AGENT_CARD, get(handle_agent_card))
            .route(ApiPaths::A2A_CARD, get(handle_agent_card))
            .with_state(state);

        let api_router = Router::new().merge(post_router).merge(get_router);

        let web_dist_path = std::path::Path::new("web/dist");
        let router = if web_dist_path.exists() {
            api_router.fallback_service(ServeDir::new(web_dist_path))
        } else {
            api_router
        };

        router.layer(CorsLayer::permissive())
    }

    pub async fn run(self) -> anyhow::Result<()> {
        self.log_server_configuration().await;
        self.start_server(None).await
    }

    pub async fn run_with_shutdown(
        self,
        shutdown_signal: impl std::future::Future<Output = ()> + Send + 'static,
    ) -> anyhow::Result<()> {
        self.log_server_configuration().await;
        self.start_server(Some(Box::pin(shutdown_signal))).await
    }

    async fn log_server_configuration(&self) {}

    async fn start_server(
        self,
        shutdown_signal: Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
    ) -> anyhow::Result<()> {
        let app = self.create_router();
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        match shutdown_signal {
            Some(signal) => axum::serve(listener, app)
                .with_graceful_shutdown(signal)
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {}", e)),
            None => axum::serve(listener, app)
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {}", e)),
        }
    }
}
