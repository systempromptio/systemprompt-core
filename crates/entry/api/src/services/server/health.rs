use axum::Json;
use serde_json::json;
use systemprompt_database::DatabaseQuery;
use systemprompt_runtime::AppContext;

pub(super) const HEALTH_CHECK_QUERY: DatabaseQuery = DatabaseQuery::new("SELECT 1");

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
pub(super) fn get_process_memory() -> Option<serde_json::Value> {
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
pub(super) fn get_process_memory() -> Option<serde_json::Value> {
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

#[allow(unsafe_code, clippy::unnecessary_cast, trivial_numeric_casts)]
fn get_disk_usage() -> Option<serde_json::Value> {
    let path = std::ffi::CString::new(".").ok()?;
    let mut stat: std::mem::MaybeUninit<libc::statvfs> = std::mem::MaybeUninit::uninit();
    // SAFETY: statvfs is a standard POSIX syscall, path is a valid CString,
    // and stat is properly initialized by the kernel on success
    let ret = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if ret != 0 {
        return None;
    }
    // SAFETY: statvfs returned 0, so stat is fully initialized
    let stat = unsafe { stat.assume_init() };

    let block_size = stat.f_frsize as u64;
    let total = stat.f_blocks as u64 * block_size;
    let available = stat.f_bavail as u64 * block_size;
    let free = stat.f_bfree as u64 * block_size;
    let used = total.saturating_sub(free);

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
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

pub(super) async fn get_system_stats(
    db: &dyn systemprompt_database::DatabaseProvider,
) -> Option<serde_json::Value> {
    let db_size_fut = db.fetch_one(&DB_SIZE_QUERY, &[]);
    let table_sizes_fut = db.fetch_all(&TABLE_SIZES_QUERY, &[]);
    let table_count_fut = db.fetch_one(&TABLE_COUNT_QUERY, &[]);
    let audit_fut = db.fetch_optional(&AUDIT_LOG_QUERY, &[]);
    let disk = get_disk_usage();

    let (db_size, table_sizes, table_count, audit) =
        tokio::join!(db_size_fut, table_sizes_fut, table_count_fut, audit_fut);

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
    use systemprompt_database::DatabaseProvider;

    let db_healthy = ctx
        .db_pool()
        .fetch_optional(&HEALTH_CHECK_QUERY, &[])
        .await
        .is_ok();

    let (status, http_status) = if db_healthy {
        ("healthy", StatusCode::OK)
    } else {
        ("unhealthy", StatusCode::SERVICE_UNAVAILABLE)
    };

    (http_status, Json(json!({ "status": status })))
}
