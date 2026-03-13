use axum::Json;
use serde_json::json;
use systemprompt_database::DatabaseQuery;
use systemprompt_models::AppPaths;
use systemprompt_runtime::AppContext;

const HEALTH_CHECK_QUERY: DatabaseQuery = DatabaseQuery::new("SELECT 1");

const DB_SIZE_QUERY: DatabaseQuery = DatabaseQuery::new(
    "SELECT pg_database_size(current_database()) as size_bytes, current_database() as db_name",
);

const TABLE_SIZES_QUERY: DatabaseQuery = DatabaseQuery::new(
    "SELECT relname as table_name, pg_total_relation_size(relid) as total_bytes, n_live_tup as \
     row_estimate FROM pg_stat_user_tables ORDER BY pg_total_relation_size(relid) DESC LIMIT 15",
);

const TABLE_COUNT_QUERY: DatabaseQuery =
    DatabaseQuery::new("SELECT COUNT(*) as count FROM pg_stat_user_tables");

const AUDIT_LOG_QUERY: DatabaseQuery = DatabaseQuery::new(
    "SELECT COUNT(*) as row_count, pg_total_relation_size('audit_log') as size_bytes, \
     MIN(created_at) as oldest, MAX(created_at) as newest FROM audit_log",
);

#[cfg(target_os = "linux")]
fn parse_proc_status_kb(content: &str, key: &str) -> Option<u64> {
    content
        .lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| {
            line.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
        })
}

#[cfg(target_os = "linux")]
fn get_process_memory() -> Option<serde_json::Value> {
    let content = std::fs::read_to_string("/proc/self/status").ok()?;

    let rss_kb = parse_proc_status_kb(&content, "VmRSS:");
    let virt_kb = parse_proc_status_kb(&content, "VmSize:");
    let peak_kb = parse_proc_status_kb(&content, "VmPeak:");

    Some(json!({
        "rss_mb": rss_kb.map(|kb| kb / 1024),
        "virtual_mb": virt_kb.map(|kb| kb / 1024),
        "peak_mb": peak_kb.map(|kb| kb / 1024)
    }))
}

#[cfg(not(target_os = "linux"))]
fn get_process_memory() -> Option<serde_json::Value> {
    None
}

#[allow(clippy::cast_precision_loss)]
fn human_bytes(bytes: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut idx = 0;
    while size >= 1024.0 && idx < UNITS.len() - 1 {
        size /= 1024.0;
        idx += 1;
    }
    format!("{size:.1} {}", UNITS[idx])
}

async fn get_disk_usage() -> Option<serde_json::Value> {
    let output = tokio::process::Command::new("df")
        .args(["-B1", "--output=size,used,avail", "."])
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().nth(1)?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let total: u64 = parts[0].parse().ok()?;
    let used: u64 = parts[1].parse().ok()?;
    let available: u64 = parts[2].parse().ok()?;

    #[allow(clippy::cast_precision_loss)]
    let usage_pct = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    #[allow(clippy::cast_possible_wrap)]
    Some(json!({
        "total": human_bytes(total as i64),
        "used": human_bytes(used as i64),
        "available": human_bytes(available as i64),
        "usage_percent": (usage_pct * 10.0).round() / 10.0
    }))
}

async fn get_system_stats(
    db: &dyn systemprompt_database::DatabaseProvider,
) -> Option<serde_json::Value> {
    let db_size_fut = db.fetch_one(&DB_SIZE_QUERY, &[]);
    let table_sizes_fut = db.fetch_all(&TABLE_SIZES_QUERY, &[]);
    let table_count_fut = db.fetch_one(&TABLE_COUNT_QUERY, &[]);
    let audit_fut = db.fetch_optional(&AUDIT_LOG_QUERY, &[]);
    let disk_fut = get_disk_usage();

    let (db_size, table_sizes, table_count, audit, disk) = tokio::join!(
        db_size_fut,
        table_sizes_fut,
        table_count_fut,
        audit_fut,
        disk_fut
    );

    let database =
        if let (Ok(size_row), Ok(tables), Ok(count_row)) = (&db_size, &table_sizes, &table_count) {
            let size_bytes = size_row
                .get("size_bytes")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let db_name = size_row
                .get("db_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let tbl_count = count_row
                .get("count")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);

            let top_tables: Vec<serde_json::Value> = tables
                .iter()
                .map(|row| {
                    let name = row
                        .get("table_name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?");
                    let total = row
                        .get("total_bytes")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(0);
                    let rows = row
                        .get("row_estimate")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(0);
                    json!({
                        "table_name": name,
                        "total_size": human_bytes(total),
                        "total_size_bytes": total,
                        "row_estimate": rows
                    })
                })
                .collect();

            Some(json!({
                "name": db_name,
                "total_size": human_bytes(size_bytes),
                "total_size_bytes": size_bytes,
                "table_count": tbl_count,
                "top_tables": top_tables
            }))
        } else {
            None
        };

    let logs = audit.ok().flatten().map(|row| {
        let row_count = row
            .get("row_count")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        let size_bytes = row
            .get("size_bytes")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        json!({
            "audit_rows": row_count,
            "audit_size": human_bytes(size_bytes),
            "audit_size_bytes": size_bytes,
            "oldest": row.get("oldest"),
            "newest": row.get("newest")
        })
    });

    Some(json!({
        "database": database,
        "disk": disk,
        "logs": logs
    }))
}

pub async fn handle_health(
    axum::extract::State(ctx): axum::extract::State<AppContext>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use systemprompt_database::{DatabaseProvider, ServiceRepository};

    let start = std::time::Instant::now();

    let (db_status, db_latency_ms) = {
        let db_start = std::time::Instant::now();
        let status = match ctx.db_pool().fetch_optional(&HEALTH_CHECK_QUERY, &[]).await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        };
        (status, db_start.elapsed().as_millis())
    };

    let (agent_count, agent_status, mcp_count, mcp_status) =
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
        };

    let web_dir = AppPaths::get()
        .map(|p| p.web().dist().to_path_buf())
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "Failed to get web dist path, using default");
            std::path::PathBuf::from("/var/www/html/dist")
        });
    let sitemap_exists = web_dir.join("sitemap.xml").exists();
    let index_exists = web_dir.join("index.html").exists();

    let db_healthy = db_status == "healthy";
    let services_ok = agent_status != "error" && mcp_status != "error";
    let content_ok = sitemap_exists && index_exists;

    let (overall_status, http_status) = match (db_healthy, services_ok && content_ok) {
        (false, _) => ("unhealthy", StatusCode::SERVICE_UNAVAILABLE),
        (true, false) => ("degraded", StatusCode::OK),
        (true, true) => ("healthy", StatusCode::OK),
    };

    let system_stats = get_system_stats(ctx.db_pool().as_ref()).await;

    let check_duration_ms = start.elapsed().as_millis();
    let memory = get_process_memory();

    let data = json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "database": {
                "status": db_status,
                "latency_ms": db_latency_ms
            },
            "agents": {
                "status": agent_status,
                "count": agent_count
            },
            "mcp": {
                "status": mcp_status,
                "count": mcp_count
            },
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
