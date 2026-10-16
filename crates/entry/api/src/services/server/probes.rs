//! Liveness and readiness probes for balancers and orchestrators.
//!
//! `/livez` answers as soon as the port is bound; `/readyz` is the admission
//! signal and answers 503 before boot completes, after the drain signal, while
//! the in-flight ceiling is saturated, and whenever the database probe fails.
//! A ready answer carries a `config` block: per declared config plane, the
//! authority, declaration hash, remaining drift and outcome of the last
//! reconcile pass. Drift never makes a replica unready.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use serde_json::json;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::repository::ConfigReconcileRepository;

use super::health::{HEALTH_CHECK_QUERY, HEALTH_PROBE_TIMEOUT};
use crate::services::middleware::LoadShed;

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
    shed: Option<axum::Extension<Arc<LoadShed>>>,
    planes: Option<axum::Extension<Arc<ConfigReconcileRepository>>>,
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

    if let Some(axum::Extension(shed)) = shed
        && shed.saturated()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "saturated",
                "in_flight": shed.in_flight(),
                "limit": shed.limit(),
                "instance": instance,
                "version": version
            })),
        );
    }

    let probe = ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]);
    let db_ready = matches!(
        tokio::time::timeout(HEALTH_PROBE_TIMEOUT, probe).await,
        Ok(Ok(_))
    );
    if db_ready {
        let config = config_block(planes.as_deref().map(AsRef::as_ref)).await;
        (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "instance": instance,
                "version": version,
                "config": config
            })),
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

async fn config_block(repository: Option<&ConfigReconcileRepository>) -> serde_json::Value {
    let Some(repository) = repository else {
        return json!({ "planes": [] });
    };
    match tokio::time::timeout(HEALTH_PROBE_TIMEOUT, repository.list_states()).await {
        Ok(Ok(rows)) => json!({
            "planes": rows.iter().map(|row| json!({
                "plane": row.plane,
                "authority": row.authority,
                "hash": row.declared_hash,
                "drift_rows": row.drift_rows,
                "outcome": row.outcome,
                "reconciled_at": row.reconciled_at,
            })).collect::<Vec<_>>()
        }),
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "readyz could not read config_plane_state");
            json!({ "planes": [], "error": "unavailable" })
        },
        Err(_) => json!({ "planes": [], "error": "timeout" }),
    }
}
