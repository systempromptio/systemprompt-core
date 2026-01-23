pub mod admin;
pub mod agent;
pub mod analytics;
pub mod content;
pub mod engagement;
pub mod mcp;
pub mod oauth;
pub mod proxy;
pub mod stream;
pub mod sync;
pub mod wellknown;

pub use agent::{artifacts_router, contexts_router, registry_router, tasks_router, webhook_router};
pub use analytics::{router as analytics_router, AnalyticsState};
pub use content::{redirect_router, router as content_router};
pub use engagement::router as engagement_router;
pub use mcp::registry_router as mcp_registry_router;
pub use oauth::{
    authenticated_router as oauth_authenticated_router, public_router as oauth_public_router,
};
pub use stream::stream_router;
pub use sync::router as sync_router;
pub use wellknown::wellknown_router;
