//! Detailed health probes.
//!
//! They report the database, service counts and static-content presence,
//! and, under `checks.extensions`, the readiness checks of every extension
//! the profile leaves enabled — the same checks `/readyz` runs. Each enabled
//! extension's non-gating `Extension::diagnostics` probes add an entry under
//! `checks.<probe name>` (the notification and brain consumer backlogs, say):
//! `{"status": "healthy", ..detail}`, or `{"status": "degraded", "error": ..}`
//! when the probe fails or overruns. The kernel names no capability here. A
//! failing check or probe reports the whole detail degraded, never a 5xx;
//! when the readiness checks or diagnostics are not mounted on this route the
//! entry says so and the detail is degraded, never healthy by default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use axum::Json;
use serde_json::json;
use systemprompt_manifest::services::ServiceModule;
use systemprompt_runtime::AppContext;

use super::health::{HEALTH_CHECK_QUERY, get_process_memory, get_system_stats};
use super::probes::ExtensionReadiness;
use systemprompt_extension::registry::ExtensionDiagnostic;
use systemprompt_extension::{DiagnosticDetail, DiagnosticFailure, DiagnosticProbe, LoaderError};

async fn check_service_counts(ctx: &AppContext) -> (usize, &'static str, usize, &'static str) {
    let service_repo = ctx.service_repository();
    let (ac, as_) = match service_repo
        .count_running_services(ServiceModule::Agent)
        .await
    {
        Ok(count) if count > 0 => (count, "healthy"),
        Ok(_) => (0, "none"),
        Err(_) => (0, "error"),
    };
    let (mc, ms) = match service_repo
        .count_running_services(ServiceModule::Mcp)
        .await
    {
        Ok(count) if count > 0 => (count, "healthy"),
        Ok(_) => (0, "none"),
        Err(_) => (0, "error"),
    };
    (ac, as_, mc, ms)
}

fn check_static_content(web_dir: &Path) -> (bool, bool) {
    (
        web_dir.join("index.html").exists(),
        web_dir.join("sitemap.xml").exists(),
    )
}

async fn probe_database(ctx: &AppContext) -> (&'static str, u128) {
    use systemprompt_database::DatabaseProvider;

    let db_start = std::time::Instant::now();
    let probe = ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]);
    let status = match tokio::time::timeout(super::health::HEALTH_PROBE_TIMEOUT, probe).await {
        Ok(Ok(_)) => "healthy",
        Ok(Err(_)) | Err(_) => "unhealthy",
    };
    (status, db_start.elapsed().as_millis())
}

// JSON: the readiness report is a free-form object per check.
async fn readiness_checks(
    readiness: Option<axum::Extension<Arc<ExtensionReadiness>>>,
) -> (bool, serde_json::Value) {
    match readiness {
        Some(axum::Extension(readiness)) => readiness.run().await,
        None => (
            false,
            json!({ "status": "degraded", "error": "extension readiness checks unavailable" }),
        ),
    }
}

// JSON: each entry is a free-form diagnostic object.
async fn diagnostic_checks(
    diagnostics: Option<axum::Extension<Arc<ExtensionDiagnostics>>>,
) -> (bool, Vec<(&'static str, serde_json::Value)>) {
    match diagnostics {
        Some(axum::Extension(diagnostics)) => diagnostics.run().await,
        None => (
            false,
            vec![(
                "diagnostics",
                json!({ "status": "degraded", "error": "extension diagnostics unavailable" }),
            )],
        ),
    }
}

pub async fn handle_health_detail(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
    readiness: Option<axum::Extension<Arc<ExtensionReadiness>>>,
    diagnostics: Option<axum::Extension<Arc<ExtensionDiagnostics>>>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;

    let start = std::time::Instant::now();

    let (db_status, db_latency_ms) = probe_database(&ctx).await;

    let (agent_count, agent_status, mcp_count, mcp_status) = check_service_counts(&ctx).await;
    let (extensions_ok, extensions) = readiness_checks(readiness).await;
    let (diagnostics_ok, diagnostic_entries) = diagnostic_checks(diagnostics).await;
    let web_dir = ctx.app_paths().web().dist().to_path_buf();
    let (memory, (index_exists, sitemap_exists)) =
        tokio::task::spawn_blocking(move || (get_process_memory(), check_static_content(&web_dir)))
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Health filesystem probes did not complete");
                (None, (false, false))
            });

    let db_healthy = db_status == "healthy";
    let services_ok = agent_status != "error" && mcp_status != "error";
    let content_ok = index_exists && sitemap_exists;
    let (overall_status, http_status) = match (
        db_healthy,
        services_ok && content_ok && extensions_ok && diagnostics_ok,
    ) {
        (false, _) => ("unhealthy", StatusCode::SERVICE_UNAVAILABLE),
        (true, false) => ("degraded", StatusCode::OK),
        (true, true) => ("healthy", StatusCode::OK),
    };

    let system_stats = get_system_stats(ctx.db_pool().as_ref()).await;
    let check_duration_ms = start.elapsed().as_millis();

    let mut data = json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "build": ctx.build_identity(),
        "checks": {
            "database": { "status": db_status, "latency_ms": db_latency_ms },
            "agents": { "status": agent_status, "count": agent_count },
            "mcp": { "status": mcp_status, "count": mcp_count },
            "static_content": {
                "status": if content_ok { "healthy" } else { "degraded" },
                "index_html": index_exists,
                "sitemap_xml": sitemap_exists
            },
            "extensions": extensions
        },
        "memory": memory,
        "system": system_stats,
        "response_time_ms": check_duration_ms
    });

    if let Some(checks) = data
        .get_mut("checks")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (name, entry) in diagnostic_entries {
            if checks.contains_key(name) {
                tracing::warn!(
                    probe = name,
                    "Diagnostic probe name collides with a core check"
                );
                continue;
            }
            checks.insert(name.to_owned(), entry);
        }
    }

    (http_status, Json(data))
}

/// The enabled extensions' non-gating diagnostic probes, reported by
/// `/health/detail` and never run by `/readyz`.
pub struct ExtensionDiagnostics {
    probes: Vec<ExtensionDiagnostic>,
}

impl std::fmt::Debug for ExtensionDiagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionDiagnostics")
            .finish_non_exhaustive()
    }
}

impl ExtensionDiagnostics {
    pub fn from_context(ctx: &AppContext) -> Result<Self, LoaderError> {
        Ok(Self {
            probes: ctx
                .extension_registry()
                .all_diagnostics(ctx, &ctx.profile().extensions.disabled)?,
        })
    }

    #[must_use]
    pub const fn from_probes(probes: Vec<ExtensionDiagnostic>) -> Self {
        Self { probes }
    }

    // JSON: each entry is a free-form diagnostic object.
    async fn run(&self) -> (bool, Vec<(&'static str, serde_json::Value)>) {
        let outcomes =
            futures_util::future::join_all(self.probes.iter().map(|(extension, probe)| {
                let extension = *extension;
                async move {
                    let outcome =
                        tokio::time::timeout(super::health::HEALTH_PROBE_TIMEOUT, probe.probe())
                            .await;
                    (
                        probe.name(),
                        diagnostic_entry(extension, probe.as_ref(), outcome),
                    )
                }
            }))
            .await;
        let mut ok = true;
        let mut entries = Vec::new();
        for (name, (healthy, entry)) in outcomes {
            ok &= healthy;
            if let Some(entry) = entry {
                entries.push((name, entry));
            }
        }
        (ok, entries)
    }
}

type ProbeOutcome = Result<Option<DiagnosticDetail>, DiagnosticFailure>;

// JSON: one `/health/detail` diagnostic entry.
fn diagnostic_entry(
    extension: &'static str,
    probe: &dyn DiagnosticProbe,
    outcome: Result<ProbeOutcome, tokio::time::error::Elapsed>,
) -> (bool, Option<serde_json::Value>) {
    match outcome {
        Ok(Ok(Some(detail))) => {
            let mut entry = serde_json::Map::new();
            entry.insert("status".to_owned(), json!("healthy"));
            entry.extend(detail);
            (true, Some(serde_json::Value::Object(entry)))
        },
        Ok(Ok(None)) => (true, None),
        Ok(Err(failure)) => {
            tracing::warn!(
                extension,
                probe = probe.name(),
                error = %failure,
                source = ?std::error::Error::source(&failure),
                "Extension diagnostic probe failed"
            );
            (
                false,
                Some(json!({ "status": "degraded", "error": failure.reason })),
            )
        },
        Err(_) => (
            false,
            Some(json!({
                "status": "degraded",
                "error": format!("{} probe timed out", probe.subject()),
            })),
        ),
    }
}

pub(crate) fn diagnostics_layer(
    ctx: &AppContext,
) -> Result<axum::Extension<Arc<ExtensionDiagnostics>>, LoaderError> {
    Ok(axum::Extension(Arc::new(
        ExtensionDiagnostics::from_context(ctx)?,
    )))
}
