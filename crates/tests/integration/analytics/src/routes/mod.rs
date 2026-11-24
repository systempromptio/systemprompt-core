//! The analytics and engagement handlers of `systemprompt-analytics-entry`
//! against the test database: event ingestion, the page-exit fan-out,
//! content-id resolution through a stub `ContentRouting`, engagement
//! recording and conversion marking. The gate, rate limit and mounting are
//! asserted on the full server in `integration/api/src/server_mount_analytics.rs`.

mod common;
mod routes_analytics;
mod routes_analytics_events_success;
mod routes_analytics_more;
mod routes_engagement;
