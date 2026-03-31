use axum::Json;
use serde_json::json;
use systemprompt_models::AppPaths;
use systemprompt_runtime::AppContext;

use super::health::{HEALTH_CHECK_QUERY, get_process_memory, get_system_stats};

async fn check_service_counts(ctx: &AppContext) -> (usize, &'static str, usize, &'static str) {
    use systemprompt_database::ServiceRepository;

    match ServiceRepository::new(ctx.db_pool()) {
        Ok(service_repo) => {
            let (ac, as_) = match service_repo.count_running_services("agent").await {
                Ok(count) if count > 0 => (count, "healthy"),
                Ok(_) => (0, "none"),
                Err(_) => (0, "error"),
            };
            let (mc, ms) = match service_repo.count_running_services("mcp").await {
                Ok(count) if count > 0 => (count, "healthy"),
                Ok(_) => (0, "none"),
                Err(_) => (0, "error"),
            };
            (ac, as_, mc, ms)
        },
        Err(_) => (0, "error", 0, "error"),
    }
}

fn check_static_content() -> (bool, bool) {
    let web_dir = AppPaths::get().map_or_else(
        |e| {
            tracing::debug!(error = %e, "Failed to get web dist path, using default");
            std::path::PathBuf::from("/var/www/html/dist")
        },
        |p| p.web().dist().to_path_buf(),
    );
    (
        web_dir.join("index.html").exists(),
        web_dir.join("sitemap.xml").exists(),
    )
}

pub async fn handle_health_detail(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use systemprompt_database::DatabaseProvider;

    let start = std::time::Instant::now();

    let (db_status, db_latency_ms) = {
        let db_start = std::time::Instant::now();
        let status = match ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]).await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        };
        (status, db_start.elapsed().as_millis())
    };

    let (agent_count, agent_status, mcp_count, mcp_status) = check_service_counts(&ctx).await;
    let (index_exists, sitemap_exists) = check_static_content();

    let db_healthy = db_status == "healthy";
    let services_ok = agent_status != "error" && mcp_status != "error";
    let content_ok = index_exists && sitemap_exists;

    let (overall_status, http_status) = match (db_healthy, services_ok && content_ok) {
        (false, _) => ("unhealthy", StatusCode::SERVICE_UNAVAILABLE),
        (true, false) => ("degraded", StatusCode::OK),
        (true, true) => ("healthy", StatusCode::OK),
    };

    let system_stats = get_system_stats(ctx.db_pool().as_ref()).await;
    let memory = get_process_memory();
    let check_duration_ms = start.elapsed().as_millis();

    // JSON: protocol boundary
    let data = json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "database": { "status": db_status, "latency_ms": db_latency_ms },
            "agents": { "status": agent_status, "count": agent_count },
            "mcp": { "status": mcp_status, "count": mcp_count },
            "static_content": {
                "status": if content_ok { "healthy" } else { "degraded" },
                "index_html": index_exists,
                "sitemap_xml": sitemap_exists
            }
        },
        "memory": memory,
        "system": system_stats,
        "response_time_ms": check_duration_ms
    });

    (http_status, Json(data))
}
