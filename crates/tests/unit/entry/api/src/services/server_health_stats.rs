//! Unit tests for the health surface's statistics formatting helpers.

use serde_json::json;
use systemprompt_api::services::server::test_api::{
    audit_log_stats, database_stats, human_bytes, table_stats,
};
use systemprompt_traits::JsonRow;

#[cfg(target_os = "linux")]
use systemprompt_api::services::server::test_api::parse_proc_status_kb;

fn row(pairs: &[(&str, serde_json::Value)]) -> JsonRow {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

#[test]
fn human_bytes_scales_units() {
    assert_eq!(human_bytes(0), "0.0 B");
    assert_eq!(human_bytes(512), "512.0 B");
    assert_eq!(human_bytes(1024), "1.0 KB");
    assert_eq!(human_bytes(1_536), "1.5 KB");
    assert_eq!(human_bytes(1_048_576), "1.0 MB");
    assert_eq!(human_bytes(1_073_741_824), "1.0 GB");
    assert_eq!(human_bytes(1_099_511_627_776), "1.0 TB");
    assert_eq!(human_bytes(1_125_899_906_842_624), "1024.0 TB");
}

#[test]
fn database_stats_shapes_summary() {
    let size_row = row(&[
        ("size_bytes", json!(2_048)),
        ("db_name", json!("systemprompt")),
    ]);
    let tables = vec![row(&[
        ("table_name", json!("users")),
        ("total_bytes", json!(1_024)),
        ("row_estimate", json!(7)),
    ])];
    let count_row = row(&[("count", json!(42))]);

    let stats = database_stats(&size_row, &tables, &count_row);
    assert_eq!(stats["name"], json!("systemprompt"));
    assert_eq!(stats["total_size"], json!("2.0 KB"));
    assert_eq!(stats["total_size_bytes"], json!(2_048));
    assert_eq!(stats["table_count"], json!(42));
    assert_eq!(stats["top_tables"][0]["table_name"], json!("users"));
    assert_eq!(stats["top_tables"][0]["row_estimate"], json!(7));
}

#[test]
fn database_stats_defaults_missing_fields() {
    let empty = JsonRow::new();
    let stats = database_stats(&empty, &[], &empty);
    assert_eq!(stats["name"], json!("unknown"));
    assert_eq!(stats["total_size_bytes"], json!(0));
    assert_eq!(stats["table_count"], json!(0));
    assert_eq!(stats["top_tables"], json!([]));
}

#[test]
fn table_stats_defaults_missing_fields() {
    let stats = table_stats(&JsonRow::new());
    assert_eq!(stats["table_name"], json!("?"));
    assert_eq!(stats["total_size"], json!("0.0 B"));
    assert_eq!(stats["row_estimate"], json!(0));
}

#[test]
fn audit_log_stats_shapes_summary() {
    let stats = audit_log_stats(&row(&[
        ("row_count", json!(5)),
        ("size_bytes", json!(4_096)),
        ("oldest", json!("2026-01-01T00:00:00Z")),
        ("newest", json!("2026-06-01T00:00:00Z")),
    ]));
    assert_eq!(stats["audit_rows"], json!(5));
    assert_eq!(stats["audit_size"], json!("4.0 KB"));
    assert_eq!(stats["audit_size_bytes"], json!(4_096));
    assert_eq!(stats["oldest"], json!("2026-01-01T00:00:00Z"));
    assert_eq!(stats["newest"], json!("2026-06-01T00:00:00Z"));
}

#[cfg(target_os = "linux")]
#[test]
fn parse_proc_status_extracts_kb_values() {
    let content = "Name:\tapi\nVmPeak:\t  204800 kB\nVmSize:\t  102400 kB\nVmRSS:\t   51200 kB\n";
    assert_eq!(parse_proc_status_kb(content, "VmRSS:"), Some(51_200));
    assert_eq!(parse_proc_status_kb(content, "VmSize:"), Some(102_400));
    assert_eq!(parse_proc_status_kb(content, "VmSwap:"), None);
    assert_eq!(
        parse_proc_status_kb("VmRSS:\tnot-a-number kB", "VmRSS:"),
        None
    );
}
