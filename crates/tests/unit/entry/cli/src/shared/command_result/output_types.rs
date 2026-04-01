#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::shared::{
    KeyValueItem, KeyValueOutput, SuccessOutput, TableOutput, TextOutput,
};

#[test]
fn test_text_output_new() {
    let output = TextOutput::new("Test message");
    assert_eq!(output.message, "Test message");
}

#[test]
fn test_text_output_new_with_string() {
    let output = TextOutput::new(String::from("String message"));
    assert_eq!(output.message, "String message");
}

#[test]
fn test_text_output_debug() {
    let output = TextOutput::new("debug test");
    let debug = format!("{:?}", output);
    assert!(debug.contains("TextOutput"));
    assert!(debug.contains("debug test"));
}

#[test]
fn test_success_output_new() {
    let output = SuccessOutput::new("Operation completed");
    assert_eq!(output.message, "Operation completed");
    assert!(output.details.is_none());
}

#[test]
fn test_success_output_with_details() {
    let output = SuccessOutput::new("Done")
        .with_details(vec!["Step 1 complete".to_string(), "Step 2 complete".to_string()]);
    assert_eq!(output.details.as_ref().unwrap().len(), 2);
}

#[test]
fn test_success_output_serialize_without_details() {
    let output = SuccessOutput::new("Done");
    let json = serde_json::to_string(&output).unwrap();
    assert_eq!(json, r#"{"message":"Done"}"#);
    assert!(!json.contains("details"));
}

#[test]
fn test_success_output_serialize_with_details() {
    let output = SuccessOutput::new("Done")
        .with_details(vec!["detail1".to_string()]);
    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("\"details\":[\"detail1\"]"));
}

#[test]
fn test_key_value_output_new() {
    let output = KeyValueOutput::new();
    assert!(output.items.is_empty());
}

#[test]
fn test_key_value_output_default() {
    let output = KeyValueOutput::default();
    assert!(output.items.is_empty());
}

#[test]
fn test_key_value_output_add_single() {
    let output = KeyValueOutput::new()
        .add("key1", "value1");
    assert_eq!(output.items.len(), 1);
    assert_eq!(output.items[0].key, "key1");
    assert_eq!(output.items[0].value, "value1");
}

#[test]
fn test_key_value_output_add_multiple() {
    let output = KeyValueOutput::new()
        .add("name", "John")
        .add("age", "30")
        .add("city", "NYC");
    assert_eq!(output.items.len(), 3);
}

#[test]
fn test_key_value_output_add_with_string() {
    let output = KeyValueOutput::new()
        .add(String::from("key"), String::from("value"));
    assert_eq!(output.items[0].key, "key");
    assert_eq!(output.items[0].value, "value");
}

#[test]
fn test_key_value_output_serialize() {
    let output = KeyValueOutput::new()
        .add("k1", "v1");
    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("\"key\":\"k1\""));
    assert!(json.contains("\"value\":\"v1\""));
}

#[test]
fn test_key_value_item_debug() {
    let item = KeyValueItem {
        key: "test_key".to_string(),
        value: "test_value".to_string(),
    };
    let debug = format!("{:?}", item);
    assert!(debug.contains("KeyValueItem"));
    assert!(debug.contains("test_key"));
}

#[test]
fn test_table_output_new() {
    let rows = vec!["row1".to_string(), "row2".to_string()];
    let output = TableOutput::new(rows);
    assert_eq!(output.rows.len(), 2);
}

#[test]
fn test_table_output_default() {
    let output: TableOutput<String> = TableOutput::default();
    assert!(output.rows.is_empty());
}

#[test]
fn test_table_output_new_empty() {
    let output: TableOutput<i32> = TableOutput::new(vec![]);
    assert!(output.rows.is_empty());
}

#[test]
fn test_table_output_with_complex_type() {
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct Row {
        id: i32,
        name: String,
    }

    let rows = vec![
        Row { id: 1, name: "Alice".to_string() },
        Row { id: 2, name: "Bob".to_string() },
    ];
    let output = TableOutput::new(rows);

    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("Alice"));
    assert!(json.contains("Bob"));
}
