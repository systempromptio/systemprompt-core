//! HTTP route modules for the API server.
//!
//! Each submodule owns one functional area (admin, agent, analytics, content,
//! engagement, gateway, marketplace, mcp, oauth, proxy, stream, sync, users,
//! wellknown) and exposes a `Router` builder. The re-exports here surface the
//! router constructors the API assembler composes into the top-level service.

pub mod admin;
pub mod agent;
pub mod analytics;
pub mod content;
pub mod engagement;
pub mod gateway;
pub mod marketplace;
pub mod mcp;
pub mod oauth;
pub mod proxy;
pub mod stream;
pub mod sync;
pub mod users;
pub mod wellknown;

pub use agent::{artifacts_router, contexts_router, registry_router, tasks_router, webhook_router};
pub use analytics::{AnalyticsState, router as analytics_router};
pub use content::{
    authenticated_router as content_authenticated_router, public_router as content_public_router,
    redirect_router,
};
pub use engagement::router as engagement_router;
pub use mcp::registry_router as mcp_registry_router;
pub use oauth::{
    authenticated_router as oauth_authenticated_router, public_router as oauth_public_router,
};
pub use stream::stream_router;
pub use sync::router as sync_router;
pub use wellknown::wellknown_router;
