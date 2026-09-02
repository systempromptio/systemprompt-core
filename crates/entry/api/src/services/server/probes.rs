//! Liveness and readiness probes for balancers and orchestrators.
//!
//! `/livez` answers as soon as the port is bound; `/readyz` is the admission
//! signal and answers 503 before boot completes, after the drain signal, and
//! whenever the database probe fails.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use serde_json::json;
use systemprompt_runtime::AppContext;

use super::health::{HEALTH_CHECK_QUERY, HEALTH_PROBE_TIMEOUT};

pub(crate) async fn handle_livez(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    Json(json!({
        "status": "alive",
        "instance": ctx.config().instance_id,
        "version": env!("CARGO_PKG_VERSION")
    }))
}

pub(crate) async fn handle_readyz(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use systemprompt_database::DatabaseProvider;

    let instance = ctx.config().instance_id.clone();
    let version = env!("CARGO_PKG_VERSION");

    if !super::readiness::is_ready() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "draining", "instance": instance, "version": version })),
        );
    }

    let probe = ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]);
    let db_ready = matches!(
        tokio::time::timeout(HEALTH_PROBE_TIMEOUT, probe).await,
        Ok(Ok(_))
    );
    if db_ready {
        (
            StatusCode::OK,
            Json(json!({ "status": "ready", "instance": instance, "version": version })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unready",
                "database": "unreachable",
                "instance": instance,
                "version": version
            })),
        )
    }
}
