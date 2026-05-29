use std::collections::HashMap;

use systemprompt_models::repository::{ServiceRecord, WhereClause};
use systemprompt_models::errors::RowParseError;

#[test]
fn where_clause_empty_build_returns_empty_string() {
    let (sql, params) = WhereClause::new().build();
    assert!(sql.is_empty());
    assert!(params.is_empty());
}

#[test]
fn where_clause_default_matches_new() {
    let (sql1, _) = WhereClause::new().build();
    let (sql2, _) = WhereClause::default().build();
    assert_eq!(sql1, sql2);
}

#[test]
fn where_clause_eq_condition() {
    let (sql, params) = WhereClause::new().eq("name", "alice").build();
    assert_eq!(sql, "WHERE name = ?");
    assert_eq!(params, vec!["alice"]);
}

#[test]
fn where_clause_multiple_conditions_joined_with_and() {
    let (sql, params) = WhereClause::new()
        .eq("status", "active")
        .eq("role", "admin")
        .build();
    assert_eq!(sql, "WHERE status = ? AND role = ?");
    assert_eq!(params, vec!["active", "admin"]);
}

#[test]
fn where_clause_not_null() {
    let (sql, params) = WhereClause::new().not_null("deleted_at").build();
    assert_eq!(sql, "WHERE deleted_at IS NOT NULL");
    assert!(params.is_empty());
}

#[test]
fn where_clause_null() {
    let (sql, params) = WhereClause::new().null("deleted_at").build();
    assert_eq!(sql, "WHERE deleted_at IS NULL");
    assert!(params.is_empty());
}

#[test]
fn where_clause_like() {
    let (sql, params) = WhereClause::new().like("email", "%@example.com").build();
    assert_eq!(sql, "WHERE email LIKE ?");
    assert_eq!(params, vec!["%@example.com"]);
}

#[test]
fn where_clause_in_list_single() {
    let (sql, params) = WhereClause::new()
        .in_list("id", vec!["1".to_owned()])
        .build();
    assert_eq!(sql, "WHERE id IN (?)");
    assert_eq!(params, vec!["1"]);
}

#[test]
fn where_clause_in_list_multiple() {
    let (sql, params) = WhereClause::new()
        .in_list("id", vec!["1".to_owned(), "2".to_owned(), "3".to_owned()])
        .build();
    assert_eq!(sql, "WHERE id IN (?, ?, ?)");
    assert_eq!(params, vec!["1", "2", "3"]);
}

#[test]
fn where_clause_mixed_conditions() {
    let (sql, params) = WhereClause::new()
        .eq("type", "user")
        .null("deleted_at")
        .like("email", "%@test.com")
        .build();
    assert_eq!(sql, "WHERE type = ? AND deleted_at IS NULL AND email LIKE ?");
    assert_eq!(params, vec!["user", "%@test.com"]);
}

fn make_row(fields: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    fields.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

#[test]
fn service_record_from_json_row_valid() {
    let row = make_row(&[
        ("name", serde_json::Value::String("api".to_owned())),
        ("module_name", serde_json::Value::String("core".to_owned())),
        ("status", serde_json::Value::String("running".to_owned())),
        ("pid", serde_json::Value::Number(1234.into())),
        ("port", serde_json::Value::Number(8080.into())),
    ]);
    let record = ServiceRecord::from_json_row(&row).unwrap();
    assert_eq!(record.name, "api");
    assert_eq!(record.module_name, "core");
    assert_eq!(record.status, "running");
    assert_eq!(record.pid, Some(1234));
    assert_eq!(record.port, 8080);
}

#[test]
fn service_record_from_json_row_no_pid() {
    let row = make_row(&[
        ("name", serde_json::Value::String("mcp".to_owned())),
        ("module_name", serde_json::Value::String("mcp".to_owned())),
        ("status", serde_json::Value::String("stopped".to_owned())),
        ("pid", serde_json::Value::Null),
        ("port", serde_json::Value::Number(5000.into())),
    ]);
    let record = ServiceRecord::from_json_row(&row).unwrap();
    assert!(record.pid.is_none());
    assert_eq!(record.port, 5000);
}

#[test]
fn service_record_from_json_row_missing_name_errors() {
    let row = make_row(&[
        ("module_name", serde_json::Value::String("core".to_owned())),
        ("status", serde_json::Value::String("running".to_owned())),
        ("port", serde_json::Value::Number(8080.into())),
    ]);
    let err = ServiceRecord::from_json_row(&row).unwrap_err();
    assert_eq!(err, RowParseError::Missing("name"));
}

#[test]
fn service_record_from_json_row_missing_port_errors() {
    let row = make_row(&[
        ("name", serde_json::Value::String("api".to_owned())),
        ("module_name", serde_json::Value::String("core".to_owned())),
        ("status", serde_json::Value::String("running".to_owned())),
    ]);
    let err = ServiceRecord::from_json_row(&row).unwrap_err();
    assert_eq!(err, RowParseError::Missing("port"));
}

#[test]
fn service_record_serde_round_trip() {
    let r = ServiceRecord {
        name: "svc".to_owned(),
        module_name: "mod".to_owned(),
        status: "running".to_owned(),
        pid: Some(99),
        port: 9000,
    };
    let json = serde_json::to_string(&r).unwrap();
    let decoded: ServiceRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.name, "svc");
    assert_eq!(decoded.pid, Some(99));
}
