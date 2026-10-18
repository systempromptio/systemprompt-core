//! Liveness and readiness probes for balancers and orchestrators.
//!
//! `/livez` answers as soon as the port is bound; `/readyz` is the admission
//! signal and answers 503 before boot completes, after the drain signal, while
//! the in-flight ceiling is saturated, and whenever the database probe fails.
//! A ready answer carries a `config` block: per declared config plane, the
//! authority, declaration hash, remaining drift and outcome of the last
//! reconcile pass, and the plane's `effect` (`live`, or `on_restart` when a
//! change reaches the runtime only after a restart; absent for a plane this
//! binary does not register). Drift never makes a replica unready.
//! It also carries `fingerprint`, this replica's configuration fingerprint,
//! so replicas running different configuration are visible.
//! A node whose database a newer release has migrated stays ready and says so
//! with `"schema": "ahead"` and the migrations it does not know, unless one of
//! them is `@compat: breaking`: then it answers 503 with `"schema":
//! "breaking"`. The ahead set is the one the node's schema watch last read,
//! not the boot snapshot.
//! Every extension readiness check runs on each probe; one that fails,
//! errors or overruns the probe timeout answers 503 with the failing checks
//! under `extensions`, so the probe fails closed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use serde_json::json;
use systemprompt_extension::{ReadinessCheck, ReadinessFailure};
use systemprompt_runtime::{AppContext, ConfigPlaneSet};
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
    readiness: Option<axum::Extension<Arc<ExtensionReadiness>>>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use systemprompt_database::DatabaseProvider;

    let instance = ctx.config().instance_id.clone();
    let fingerprint = super::config_fingerprint::current();
    let respond = |status: StatusCode, mut body: serde_json::Value, config: serde_json::Value| {
        if let Some(fields) = body.as_object_mut() {
            fields.insert("instance".to_owned(), json!(instance));
            fields.insert("version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
            fields.insert("config".to_owned(), config);
        }
        (status, Json(body))
    };
    let bare = json!({ "fingerprint": fingerprint });

    if !super::readiness::is_ready() {
        return respond(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "status": "draining" }),
            bare,
        );
    }

    if let Some(axum::Extension(shed)) = shed
        && shed.saturated()
    {
        let body = json!({
            "status": "saturated",
            "in_flight": shed.in_flight(),
            "limit": shed.limit(),
        });
        return respond(StatusCode::SERVICE_UNAVAILABLE, body, bare);
    }

    let probe = ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]);
    let db_ready = matches!(
        tokio::time::timeout(HEALTH_PROBE_TIMEOUT, probe).await,
        Ok(Ok(_))
    );
    if !db_ready {
        let body = json!({ "status": "unready", "database": "unreachable" });
        return respond(StatusCode::SERVICE_UNAVAILABLE, body, bare);
    }

    let registered = registered_planes(&ctx);
    let config = config_block(
        fingerprint,
        planes.as_deref().map(AsRef::as_ref),
        registered.as_ref(),
    )
    .await;
    let (extensions_ready, extensions) = match readiness.as_deref() {
        Some(readiness) => readiness.run().await,
        None => (true, json!([])),
    };
    if !extensions_ready {
        let body = json!({ "status": "unready", "extensions": extensions });
        return respond(StatusCode::SERVICE_UNAVAILABLE, body, config);
    }
    let schema = ctx.schema_state();
    let ahead = schema.ahead();
    if schema.breaks_this_release() {
        let body = json!({ "status": "unready", "schema": "breaking", "schema_ahead": ahead });
        return respond(StatusCode::SERVICE_UNAVAILABLE, body, config);
    }
    let body = if ahead.is_empty() {
        json!({ "status": "ready", "extensions": extensions })
    } else {
        json!({
            "status": "ready",
            "schema": "ahead",
            "schema_ahead": ahead,
            "extensions": extensions,
        })
    };
    respond(StatusCode::OK, body, config)
}

// JSON: the `/readyz` body is assembled as an open JSON object.
async fn config_block(
    fingerprint: Option<systemprompt_config::ConfigFingerprint>,
    repository: Option<&ConfigReconcileRepository>,
    registered: Option<&ConfigPlaneSet>,
) -> serde_json::Value {
    let Some(repository) = repository else {
        return json!({ "fingerprint": fingerprint, "planes": [] });
    };
    match tokio::time::timeout(HEALTH_PROBE_TIMEOUT, repository.list_states()).await {
        Ok(Ok(rows)) => json!({
            "fingerprint": fingerprint,
            "planes": rows.iter().map(|row| json!({
                "plane": row.plane,
                "authority": row.authority,
                "effect": registered
                    .and_then(|set| set.find(&row.plane).ok())
                    .map(|plane| plane.effect().as_str()),
                "hash": row.declared_hash,
                "drift_rows": row.drift_rows,
                "outcome": row.outcome,
                "reconciled_at": row.reconciled_at,
            })).collect::<Vec<_>>()
        }),
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "readyz could not read config_plane_state");
            json!({ "fingerprint": fingerprint, "planes": [], "error": "unavailable" })
        },
        Err(_) => json!({ "fingerprint": fingerprint, "planes": [], "error": "timeout" }),
    }
}

pub(crate) struct ExtensionReadiness {
    checks: Vec<(&'static str, Arc<dyn ReadinessCheck>)>,
}

impl ExtensionReadiness {
    pub(crate) fn from_context(ctx: &AppContext) -> Self {
        Self {
            checks: ctx.extension_registry().all_readiness_checks(ctx),
        }
    }

    // JSON: the per-check `/readyz` entries are an open JSON array.
    async fn run(&self) -> (bool, serde_json::Value) {
        let outcomes =
            futures_util::future::join_all(self.checks.iter().map(|(extension, check)| {
                let extension = *extension;
                async move {
                    let outcome = tokio::time::timeout(HEALTH_PROBE_TIMEOUT, check.check()).await;
                    check_entry(extension, check.name(), outcome)
                }
            }))
            .await;
        let ready = outcomes.iter().all(|(ok, _)| *ok);
        let entries = outcomes.into_iter().map(|(_, entry)| entry).collect();
        (ready, serde_json::Value::Array(entries))
    }
}

fn registered_planes(ctx: &AppContext) -> Option<ConfigPlaneSet> {
    match ctx.config_planes() {
        Ok(planes) => Some(planes),
        Err(error) => {
            tracing::warn!(%error, "config plane set unavailable for /readyz");
            None
        },
    }
}

pub(crate) fn readiness_layer(ctx: &AppContext) -> axum::Extension<Arc<ExtensionReadiness>> {
    axum::Extension(Arc::new(ExtensionReadiness::from_context(ctx)))
}

// JSON: one entry of the outgoing `/readyz` extensions array.
fn check_entry(
    extension: &'static str,
    check: &'static str,
    outcome: Result<Result<(), ReadinessFailure>, tokio::time::error::Elapsed>,
) -> (bool, serde_json::Value) {
    match outcome {
        Ok(Ok(())) => (
            true,
            json!({ "extension": extension, "check": check, "status": "ok" }),
        ),
        Ok(Err(ReadinessFailure::Unready { reason })) => (
            false,
            json!({ "extension": extension, "check": check, "status": "unready", "reason": reason }),
        ),
        Ok(Err(error)) => {
            tracing::warn!(extension, check, error = %error, "extension readiness check errored");
            (
                false,
                json!({ "extension": extension, "check": check, "status": "error" }),
            )
        },
        Err(_) => (
            false,
            json!({ "extension": extension, "check": check, "status": "timeout" }),
        ),
    }
}
