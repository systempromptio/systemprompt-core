//! Liveness and readiness probes for balancers and orchestrators.
//!
//! `/livez` answers as soon as the port is bound; `/readyz` is the admission
//! signal and answers 503 before boot completes, after the drain signal, while
//! the in-flight ceiling is saturated, and whenever the database probe fails.
//! A ready answer carries a `config` block: per declared config plane, the
//! authority, declaration hash, remaining drift and outcome of the last
//! reconcile pass, and the plane's `effect` (`live`, or `on_restart` when a
//! change reaches the runtime only after a restart; absent for a plane this
//! binary does not register). Drift never makes a replica unready. A replica
//! whose governance plane was never applied says so under `config.unapplied`
//! with the profile's `unapplied_planes` mode, its effect and the command that
//! applies it; it stays ready either way.
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

use crate::error::ApiHttpError;
use axum::Json;
use axum::http::StatusCode;
use serde_json::json;
use systemprompt_database::{DbPool, PLANE_NOT_APPLIED_HINT};
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_loader::BootContext;
use systemprompt_manifest::Config;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::{AppContext, ConfigPlaneSet, SchemaState};
use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_traits::ConfigWatch;

use super::extension_readiness::ExtensionReadiness;
use super::health::{HEALTH_CHECK_QUERY, HEALTH_PROBE_TIMEOUT};
use super::readiness::ReadinessState;
use crate::services::middleware::LoadShed;

pub(crate) async fn handle_livez(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> Result<Json<serde_json::Value>, ApiHttpError> {
    let mut body = serde_json::Map::new();
    body.insert("status".to_owned(), json!("alive"));
    body.extend(super::identity::node_identity(&ctx).map_err(seed_error)?);
    Ok(Json(serde_json::Value::Object(body)))
}

// JSON: the `/readyz` body of a node that refuses traffic.
fn refusing_traffic(
    api_ready: &ReadinessState,
    shed: Option<&LoadShed>,
) -> Option<serde_json::Value> {
    if !api_ready.is_ready() {
        return Some(json!({ "status": "draining" }));
    }
    shed.filter(|shed| shed.saturated()).map(|shed| {
        json!({
            "status": "saturated",
            "in_flight": shed.in_flight(),
            "limit": shed.limit(),
        })
    })
}

pub(super) fn seed_error(error: MissingDependency) -> ApiHttpError {
    ApiHttpError::internal("node state", error)
}

pub(crate) async fn handle_readyz(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
    shed: Option<axum::Extension<Arc<LoadShed>>>,
    planes: Option<axum::Extension<Arc<ConfigReconcileRepository>>>,
    readiness: Option<axum::Extension<Arc<ExtensionReadiness>>>,
    axum::Extension(api_ready): axum::Extension<Arc<ReadinessState>>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiHttpError> {
    use systemprompt_database::DatabaseProvider;

    let state = ctx.kernel().state();
    let identity = super::identity::node_identity(&ctx).map_err(seed_error)?;
    let fingerprint =
        super::config_fingerprint::current(state.get::<BootContext>().map_err(seed_error)?);
    let respond = |status: StatusCode, mut body: serde_json::Value, config: serde_json::Value| {
        if let Some(fields) = body.as_object_mut() {
            fields.extend(identity.clone());
            fields.insert("config".to_owned(), config);
        }
        Ok((status, Json(body)))
    };
    let bare = json!({ "fingerprint": fingerprint });

    if let Some(body) = refusing_traffic(&api_ready, shed.as_deref().map(AsRef::as_ref)) {
        return respond(StatusCode::SERVICE_UNAVAILABLE, body, bare);
    }

    let probe = state
        .get::<DbPool>()
        .map_err(seed_error)?
        .fetch_optional(&HEALTH_CHECK_QUERY, &[]);
    let db_ready = matches!(
        tokio::time::timeout(HEALTH_PROBE_TIMEOUT, probe).await,
        Ok(Ok(_))
    );
    if !db_ready {
        let body = json!({ "status": "unready", "database": "unreachable" });
        return respond(StatusCode::SERVICE_UNAVAILABLE, body, bare);
    }

    let registered = registered_planes(&ctx).map_err(seed_error)?;
    let mut config = config_block(
        fingerprint,
        planes.as_deref().map(AsRef::as_ref),
        registered.as_ref(),
    )
    .await;
    flag_unapplied_governance(&ctx, &mut config).map_err(seed_error)?;
    flag_degraded_services_source(&ctx, &mut config).map_err(seed_error)?;
    let (extensions_ready, extensions) = match readiness.as_deref() {
        Some(readiness) => readiness.run().await,
        None => (true, json!([])),
    };
    if !extensions_ready {
        let body = json!({ "status": "unready", "extensions": extensions });
        return respond(StatusCode::SERVICE_UNAVAILABLE, body, config);
    }
    let schema = state.get::<Arc<SchemaState>>().map_err(seed_error)?;
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

// JSON: the `/readyz` config block is an open JSON object.
fn flag_unapplied_governance(
    ctx: &AppContext,
    config: &mut serde_json::Value,
) -> Result<(), MissingDependency> {
    if let (Some(block), Some(fields)) = (unapplied_governance(ctx)?, config.as_object_mut()) {
        fields.insert("unapplied".to_owned(), block);
    }
    Ok(())
}

// JSON: the `/readyz` config block is an open JSON object.
fn flag_degraded_services_source(
    ctx: &AppContext,
    config: &mut serde_json::Value,
) -> Result<(), MissingDependency> {
    if let (Some(block), Some(fields)) = (degraded_services_source(ctx)?, config.as_object_mut()) {
        fields.insert("services_source".to_owned(), block);
    }
    Ok(())
}

// JSON: `/readyz` and `/health` carry this block as an open JSON object.
pub(super) fn degraded_services_source(
    ctx: &AppContext,
) -> Result<Option<serde_json::Value>, MissingDependency> {
    use systemprompt_loader::bundle::{BundleCache, cache_root};
    use systemprompt_manifest::services::root::ServicesProvenance;
    let boot = ctx.kernel().state().get::<BootContext>()?;
    let Some(root) = boot.services_root() else {
        return Ok(None);
    };
    Ok(match &root.provenance {
        ServicesProvenance::LastGood { error, .. } => Some(json!({
            "serving": "last_good",
            "error": error,
        })),
        ServicesProvenance::BundledFallback { error } => Some(json!({
            "serving": "bundled",
            "error": error,
        })),
        ServicesProvenance::Bundled | ServicesProvenance::Fetched { .. } => {
            let profile = boot.profile();
            crate::routes::admin::services::held_back_block(
                !profile.services.sources.is_empty(),
                &BundleCache::new(cache_root(profile)),
            )
        },
    })
}

// JSON: `/readyz` and `/health` carry this block as an open JSON object.
pub(super) fn unapplied_governance(
    ctx: &AppContext,
) -> Result<Option<serde_json::Value>, MissingDependency> {
    let governance = ctx
        .kernel()
        .state()
        .get::<ConfigWatch<GovernanceEngine>>()?
        .get();
    let Some(mode) = governance.unapplied_mode() else {
        return Ok(None);
    };
    let effect = if mode.denies() {
        "every governed request is denied"
    } else {
        "governed requests pass ungoverned, with a warning on each"
    };
    Ok(Some(json!({
        "planes": ["governance"],
        "unapplied_planes": mode.as_str(),
        "effect": effect,
        "fix": PLANE_NOT_APPLIED_HINT,
    })))
}

fn registered_planes(ctx: &AppContext) -> Result<Option<ConfigPlaneSet>, MissingDependency> {
    let config = ctx.kernel().state().get::<Arc<Config>>()?;
    Ok(
        match systemprompt_runtime::config_plane_set(ctx.admitted(), config) {
            Ok(planes) => Some(planes),
            Err(error) => {
                tracing::warn!(%error, "config plane set unavailable for /readyz");
                None
            },
        },
    )
}
