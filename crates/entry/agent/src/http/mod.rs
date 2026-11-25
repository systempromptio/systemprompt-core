//! The agent HTTP surface: the conversation route sets
//! [`crate::extension::AgentHttpExtension`] contributes.
//!
//! | base | policy | context | rate limit | routes |
//! |---|---|---|---|---|
//! | `/api/v1/core/contexts` | user | user | `contexts` | the caller's contexts, their tasks, artifacts, notifications and events |
//! | `/api/v1/core/tasks` | user | user | `tasks` | the caller's tasks, their messages and artifacts |
//! | `/api/v1/core/artifacts` | user | user | `artifacts` | the caller's artifacts and their MCP App UI |
//! | `/api/v1/agents/registry` | public | public | `agent_registry` | `GET /` only: the agent cards |
//!
//! Every set has the default body limit and is in the agent role group. The
//! three `/api/v1/core` bases are nested in the kernel's `/api/v1/core`
//! namespace and held segment by segment; the registry is nested in the
//! kernel's `/api/v1/agents` proxy namespace, so its conferral pins the set
//! to `GET /` and the reserved entry holds `/api/v1/agents/registry`
//! exactly. Every accessor checks ownership against the authenticated caller
//! before it returns data.
//!
//! [`AgentHttpSurface`] builds the routers the API mounts with its own
//! layering: the webhook broadcasts (`/api/v1/webhook`), the SSE streams
//! (`/api/v1/stream`) and the well-known agent cards.
//!
//! Handlers read [`AgentHttpState`] (repositories built from the database
//! handle, the [`ContextEventPublisher`] the member provides, and the UI
//! renderer registry when the `mcp` member provides one) or
//! [`AgentRegistryState`] (the services view, the service table and the
//! open [`AgentCardSite`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_cards;
pub mod artifacts;
pub mod contexts;
mod error;
pub mod events;
pub mod registry;
pub mod responses;
pub mod stream;
mod surface;
pub mod tasks;
pub mod webhook;

pub use surface::AgentHttpSurface;

use std::sync::Arc;

use axum::http::Method;
use systemprompt_agent::services::registry::AgentCardSite;
use systemprompt_database::{DbPool, ServiceRepository};

pub use crate::repository::ConversationRepositories;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, FromState, MissingDependency, RateBudget, RateLimit, Resolved,
    RouteGroup,
};
use systemprompt_identifiers::{ContextId, InstanceId};
use systemprompt_manifest::services::ServicesView;
use systemprompt_extension::artifact_ui::DynArtifactUiRenderer;
use systemprompt_models::api::ApiError;
use systemprompt_traits::DynSessionUsageCounters;

pub use error::AgentHttpError;
pub use events::ContextEventPublisher;

const DEFAULT_PAGE_LIMIT: u16 = 100;
const MAX_PAGE_LIMIT: u16 = 500;

const fn user_spec(rate: RateLimit) -> RouteSpec {
    RouteSpec::new(
        AuthzPolicy::user(),
        ContextKind::User,
        RouteLimits {
            rate,
            body: BodyLimit::Default,
        },
    )
}

const CONTEXTS: RouteSpec = user_spec(RateLimit::new("contexts", RateBudget::Contexts));
const TASKS: RouteSpec = user_spec(RateLimit::new("tasks", RateBudget::Tasks));
const ARTIFACTS: RouteSpec = user_spec(RateLimit::new("artifacts", RateBudget::Artifacts));
const REGISTRY: RouteSpec = RouteSpec::new(
    AuthzPolicy::public(),
    ContextKind::Public,
    RouteLimits {
        rate: RateLimit::new("agent_registry", RateBudget::AgentRegistry),
        body: BodyLimit::Default,
    },
);

pub(crate) fn clamp_page_limit(requested: Option<u32>) -> u16 {
    requested.map_or(DEFAULT_PAGE_LIMIT, |limit| {
        u16::try_from(limit).map_or(MAX_PAGE_LIMIT, |l| l.clamp(1, MAX_PAGE_LIMIT))
    })
}

pub(crate) fn parse_context_id(raw: &str) -> Result<ContextId, AgentHttpError> {
    ContextId::try_new(raw).map_err(|e| AgentHttpError::from(ApiError::from(e)))
}

/// Router-scoped state of the contexts, tasks and artifacts routes.
#[derive(Clone)]
pub struct AgentHttpState {
    pub(crate) repositories: Arc<ConversationRepositories>,
    pub(crate) events: Arc<ContextEventPublisher>,
    pub(crate) renderers: Option<DynArtifactUiRenderer>,
}

impl std::fmt::Debug for AgentHttpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHttpState")
            .field("renderers", &self.renderers.is_some())
            .finish_non_exhaustive()
    }
}

impl AgentHttpState {
    #[must_use]
    pub const fn new(
        repositories: Arc<ConversationRepositories>,
        events: Arc<ContextEventPublisher>,
        renderers: Option<DynArtifactUiRenderer>,
    ) -> Self {
        Self {
            repositories,
            events,
            renderers,
        }
    }
}

impl FromState for AgentHttpState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let db = r.get::<DbPool>()?;
        let sessions = r.get::<DynSessionUsageCounters>()?;
        Ok(Self::new(
            Arc::new(ConversationRepositories::new(&db, sessions)),
            r.get::<Arc<ContextEventPublisher>>()?,
            r.optional::<DynArtifactUiRenderer>(),
        ))
    }
}

/// Router-scoped state of the agent registry route.
#[derive(Clone)]
pub struct AgentRegistryState {
    pub(crate) services: ServicesView,
    pub(crate) service_table: Arc<ServiceRepository>,
    pub(crate) site: Arc<AgentCardSite>,
}

impl std::fmt::Debug for AgentRegistryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRegistryState")
            .field("site", &self.site)
            .finish_non_exhaustive()
    }
}

impl AgentRegistryState {
    #[must_use]
    pub const fn new(
        services: ServicesView,
        service_table: Arc<ServiceRepository>,
        site: Arc<AgentCardSite>,
    ) -> Self {
        Self {
            services,
            service_table,
            site,
        }
    }
}

impl FromState for AgentRegistryState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let db = r.get::<DbPool>()?;
        Ok(Self::new(
            r.get::<ServicesView>()?,
            crate::repository::service_table(&db, r.get::<InstanceId>()?),
            r.get::<Arc<AgentCardSite>>()?,
        ))
    }
}

#[must_use]
pub fn contexts_routes() -> RouteSet {
    RouteSet::at::<AgentHttpState>(BasePath::Api(ApiBase::CoreContexts), RouteGroup::Agent)
        .route(Method::GET, "/", contexts::list_contexts, CONTEXTS)
        .route(Method::POST, "/", contexts::create_context, CONTEXTS)
        .route(Method::GET, "/{id}", contexts::get_context, CONTEXTS)
        .route(Method::PUT, "/{id}", contexts::update_context, CONTEXTS)
        .route(Method::DELETE, "/{id}", contexts::delete_context, CONTEXTS)
        .route(
            Method::GET,
            "/{context_id}/tasks",
            tasks::list_tasks_by_context,
            CONTEXTS,
        )
        .route(
            Method::GET,
            "/{context_id}/artifacts",
            artifacts::list_artifacts_by_context,
            CONTEXTS,
        )
        .route(
            Method::POST,
            "/{context_id}/notifications",
            contexts::handle_context_notification,
            CONTEXTS,
        )
        .route(
            Method::POST,
            "/{context_id}/events",
            contexts::events::forward_event,
            CONTEXTS,
        )
        .build()
}

#[must_use]
pub fn tasks_routes() -> RouteSet {
    RouteSet::at::<AgentHttpState>(BasePath::Api(ApiBase::CoreTasks), RouteGroup::Agent)
        .route(Method::GET, "/", tasks::list_tasks_by_user, TASKS)
        .route(Method::GET, "/{task_id}", tasks::get_task, TASKS)
        .route(Method::DELETE, "/{task_id}", tasks::delete_task, TASKS)
        .route(
            Method::GET,
            "/{task_id}/messages",
            tasks::get_messages_by_task,
            TASKS,
        )
        .route(
            Method::GET,
            "/{task_id}/artifacts",
            artifacts::list_artifacts_by_task,
            TASKS,
        )
        .build()
}

#[must_use]
pub fn artifacts_routes() -> RouteSet {
    RouteSet::at::<AgentHttpState>(BasePath::Api(ApiBase::CoreArtifacts), RouteGroup::Agent)
        .route(
            Method::GET,
            "/",
            artifacts::list_artifacts_by_user,
            ARTIFACTS,
        )
        .route(
            Method::GET,
            "/{artifact_id}",
            artifacts::get_artifact,
            ARTIFACTS,
        )
        .route(
            Method::GET,
            "/{artifact_id}/ui",
            artifacts::get_artifact_ui,
            ARTIFACTS,
        )
        .build()
}

#[must_use]
pub fn registry_routes() -> RouteSet {
    RouteSet::at::<AgentRegistryState>(BasePath::Api(ApiBase::AgentsRegistry), RouteGroup::Agent)
        .route(Method::GET, "/", registry::handle_agent_registry, REGISTRY)
        .build()
}
