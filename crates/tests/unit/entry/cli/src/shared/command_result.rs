//! Unit tests for command result types
//!
//! Tests cover:
//! - ArtifactType enum variants
//! - ChartType enum variants
//! - RenderingHints default and construction
//! - CommandResult builder pattern
//! - CommandResult factory methods
//! - TextOutput construction
//! - SuccessOutput construction and builder
//! - KeyValueOutput builder pattern
//! - TableOutput construction

#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::shared::{
    ArtifactType, ChartType, CommandResult, KeyValueItem, KeyValueOutput,
    RenderingHints, SuccessOutput, TableOutput, TextOutput,
};

// ============================================================================
// ArtifactType Tests
// ============================================================================

#[test]
fn test_artifact_type_table_variant() {
    let artifact = ArtifactType::Table;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"table\"");
}

#[test]
fn test_artifact_type_list_variant() {
    let artifact = ArtifactType::List;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"list\"");
}

#[test]
fn test_artifact_type_presentation_card_variant() {
    let artifact = ArtifactType::PresentationCard;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"presentation_card\"");
}

#[test]
fn test_artifact_type_text_variant() {
    let artifact = ArtifactType::Text;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"text\"");
}

#[test]
fn test_artifact_type_copy_paste_text_variant() {
    let artifact = ArtifactType::CopyPasteText;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"copy_paste_text\"");
}

#[test]
fn test_artifact_type_chart_variant() {
    let artifact = ArtifactType::Chart;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"chart\"");
}

#[test]
fn test_artifact_type_form_variant() {
    let artifact = ArtifactType::Form;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"form\"");
}

#[test]
fn test_artifact_type_dashboard_variant() {
    let artifact = ArtifactType::Dashboard;
    let json = serde_json::to_string(&artifact).unwrap();
    assert_eq!(json, "\"dashboard\"");
}

#[test]
fn test_artifact_type_deserialize() {
    let artifact: ArtifactType = serde_json::from_str("\"table\"").unwrap();
    assert!(matches!(artifact, ArtifactType::Table));
}

#[test]
fn test_artifact_type_clone() {
    let original = ArtifactType::Chart;
    let cloned = original;
    assert!(matches!(cloned, ArtifactType::Chart));
}

#[test]
fn test_artifact_type_debug() {
    let artifact = ArtifactType::Dashboard;
    let debug = format!("{:?}", artifact);
    assert!(debug.contains("Dashboard"));
}

// ============================================================================
// ChartType Tests
// ============================================================================

#[test]
fn test_chart_type_bar_variant() {
    let chart = ChartType::Bar;
    let json = serde_json::to_string(&chart).unwrap();
    assert_eq!(json, "\"bar\"");
}

#[test]
fn test_chart_type_line_variant() {
    let chart = ChartType::Line;
    let json = serde_json::to_string(&chart).unwrap();
    assert_eq!(json, "\"line\"");
}

#[test]
fn test_chart_type_pie_variant() {
    let chart = ChartType::Pie;
    let json = serde_json::to_string(&chart).unwrap();
    assert_eq!(json, "\"pie\"");
}

#[test]
fn test_chart_type_area_variant() {
    let chart = ChartType::Area;
    let json = serde_json::to_string(&chart).unwrap();
    assert_eq!(json, "\"area\"");
}

#[test]
fn test_chart_type_deserialize() {
    let chart: ChartType = serde_json::from_str("\"line\"").unwrap();
    assert!(matches!(chart, ChartType::Line));
}

#[test]
fn test_chart_type_clone() {
    let original = ChartType::Pie;
    let cloned = original;
    assert!(matches!(cloned, ChartType::Pie));
}

#[test]
fn test_chart_type_debug() {
    let chart = ChartType::Area;
    let debug = format!("{:?}", chart);
    assert!(debug.contains("Area"));
}

// ============================================================================
// RenderingHints Tests
// ============================================================================

#[test]
fn test_rendering_hints_default() {
    let hints = RenderingHints::default();
    assert!(hints.columns.is_none());
    assert!(hints.chart_type.is_none());
    assert!(hints.theme.is_none());
    assert!(hints.extra.is_empty());
}

#[test]
fn test_rendering_hints_with_columns() {
    let hints = RenderingHints {
        columns: Some(vec!["name".to_string(), "value".to_string()]),
        ..Default::default()
    };
    assert_eq!(hints.columns.as_ref().unwrap().len(), 2);
}

#[test]
fn test_rendering_hints_with_chart_type() {
    let hints = RenderingHints {
        chart_type: Some(ChartType::Bar),
        ..Default::default()
    };
    assert!(matches!(hints.chart_type, Some(ChartType::Bar)));
}

#[test]
fn test_rendering_hints_with_theme() {
    let hints = RenderingHints {
        theme: Some("dark".to_string()),
        ..Default::default()
    };
    assert_eq!(hints.theme.as_ref().unwrap(), "dark");
}

#[test]
fn test_rendering_hints_serialize_skip_none() {
    let hints = RenderingHints::default();
    let json = serde_json::to_string(&hints).unwrap();
    assert!(!json.contains("columns"));
    assert!(!json.contains("chart_type"));
    assert!(!json.contains("theme"));
}

#[test]
fn test_rendering_hints_serialize_with_values() {
    let hints = RenderingHints {
        columns: Some(vec!["col1".to_string()]),
        chart_type: Some(ChartType::Line),
        theme: Some("light".to_string()),
        extra: Default::default(),
    };
    let json = serde_json::to_string(&hints).unwrap();
    assert!(json.contains("columns"));
    assert!(json.contains("chart_type"));
    assert!(json.contains("theme"));
}

#[test]
fn test_rendering_hints_clone() {
    let original = RenderingHints {
        columns: Some(vec!["test".to_string()]),
        ..Default::default()
    };
    let cloned = original.clone();
    assert_eq!(cloned.columns, original.columns);
}

// ============================================================================
// CommandResult Factory Method Tests
// ============================================================================

#[test]
fn test_command_result_table() {
    let result: CommandResult<String> = CommandResult::table("test data".to_string());
    assert!(matches!(result.artifact_type, ArtifactType::Table));
    assert_eq!(result.data, "test data");
    assert!(result.title.is_none());
    assert!(result.hints.is_none());
}

#[test]
fn test_command_result_list() {
    let result: CommandResult<Vec<i32>> = CommandResult::list(vec![1, 2, 3]);
    assert!(matches!(result.artifact_type, ArtifactType::List));
    assert_eq!(result.data.len(), 3);
}

#[test]
fn test_command_result_card() {
    let result: CommandResult<&str> = CommandResult::card("card content");
    assert!(matches!(result.artifact_type, ArtifactType::PresentationCard));
}

#[test]
fn test_command_result_text() {
    let result: CommandResult<&str> = CommandResult::text("plain text");
    assert!(matches!(result.artifact_type, ArtifactType::Text));
}

#[test]
fn test_command_result_copy_paste() {
    let result: CommandResult<&str> = CommandResult::copy_paste("copyable text");
    assert!(matches!(result.artifact_type, ArtifactType::CopyPasteText));
}

#[test]
fn test_command_result_chart() {
    let result: CommandResult<Vec<i32>> = CommandResult::chart(vec![10, 20, 30], ChartType::Bar);
    assert!(matches!(result.artifact_type, ArtifactType::Chart));
    assert!(result.hints.is_some());
    assert!(matches!(
        result.hints.as_ref().unwrap().chart_type,
        Some(ChartType::Bar)
    ));
}

#[test]
fn test_command_result_form() {
    let result: CommandResult<&str> = CommandResult::form("form data");
    assert!(matches!(result.artifact_type, ArtifactType::Form));
}

#[test]
fn test_command_result_dashboard() {
    let result: CommandResult<&str> = CommandResult::dashboard("dashboard data");
    assert!(matches!(result.artifact_type, ArtifactType::Dashboard));
}

// ============================================================================
// CommandResult Builder Tests
// ============================================================================

#[test]
fn test_command_result_with_title() {
    let result = CommandResult::table("data")
        .with_title("My Title");
    assert_eq!(result.title.as_ref().unwrap(), "My Title");
}

#[test]
fn test_command_result_with_title_into_string() {
    let result = CommandResult::table("data")
        .with_title(String::from("String Title"));
    assert_eq!(result.title.as_ref().unwrap(), "String Title");
}

#[test]
fn test_command_result_with_hints() {
    let hints = RenderingHints {
        theme: Some("dark".to_string()),
        ..Default::default()
    };
    let result = CommandResult::table("data")
        .with_hints(hints);
    assert!(result.hints.is_some());
    assert_eq!(result.hints.as_ref().unwrap().theme.as_ref().unwrap(), "dark");
}

#[test]
fn test_command_result_with_columns() {
    let result = CommandResult::table("data")
        .with_columns(vec!["col1".to_string(), "col2".to_string()]);
    assert!(result.hints.is_some());
    let columns = result.hints.as_ref().unwrap().columns.as_ref().unwrap();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0], "col1");
    assert_eq!(columns[1], "col2");
}

#[test]
fn test_command_result_with_columns_preserves_existing_hints() {
    let hints = RenderingHints {
        theme: Some("dark".to_string()),
        ..Default::default()
    };
    let result = CommandResult::table("data")
        .with_hints(hints)
        .with_columns(vec!["col1".to_string()]);

    let final_hints = result.hints.as_ref().unwrap();
    assert_eq!(final_hints.theme.as_ref().unwrap(), "dark");
    assert!(final_hints.columns.is_some());
}

#[test]
fn test_command_result_skip_render() {
    let result = CommandResult::table("data");
    assert!(!result.should_skip_render());

    let result = result.with_skip_render();
    assert!(result.should_skip_render());
}

#[test]
fn test_command_result_builder_chain() {
    let result = CommandResult::chart(vec![1, 2, 3], ChartType::Line)
        .with_title("Sales Data")
        .with_columns(vec!["month".to_string(), "value".to_string()]);

    assert!(matches!(result.artifact_type, ArtifactType::Chart));
    assert_eq!(result.title.as_ref().unwrap(), "Sales Data");
    assert!(result.hints.is_some());
}

// ============================================================================
// CommandResult Serialization Tests
// ============================================================================

#[test]
fn test_command_result_serialize_basic() {
    let result = CommandResult::text("hello");
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"data\":\"hello\""));
    assert!(json.contains("\"artifact_type\":\"text\""));
}

#[test]
fn test_command_result_serialize_with_title() {
    let result = CommandResult::text("hello").with_title("Greeting");
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"title\":\"Greeting\""));
}

#[test]
fn test_command_result_serialize_skips_none_fields() {
    let result = CommandResult::text("hello");
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("\"title\":"));
    assert!(!json.contains("\"hints\":"));
}

#[test]
fn test_command_result_deserialize() {
    let json = r#"{"data":"test","artifact_type":"table"}"#;
    let result: CommandResult<String> = serde_json::from_str(json).unwrap();
    assert_eq!(result.data, "test");
    assert!(matches!(result.artifact_type, ArtifactType::Table));
}

// ============================================================================
// TextOutput Tests
// ============================================================================

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
fn test_text_output_serialize() {
    let output = TextOutput::new("hello");
    let json = serde_json::to_string(&output).unwrap();
    assert_eq!(json, r#"{"message":"hello"}"#);
}

#[test]
fn test_text_output_deserialize() {
    let json = r#"{"message":"world"}"#;
    let output: TextOutput = serde_json::from_str(json).unwrap();
    assert_eq!(output.message, "world");
}

#[test]
fn test_text_output_clone() {
    let original = TextOutput::new("original");
    let cloned = original.clone();
    assert_eq!(cloned.message, "original");
}

#[test]
fn test_text_output_debug() {
    let output = TextOutput::new("debug test");
    let debug = format!("{:?}", output);
    assert!(debug.contains("TextOutput"));
    assert!(debug.contains("debug test"));
}

// ============================================================================
// SuccessOutput Tests
// ============================================================================

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
fn test_success_output_clone() {
    let original = SuccessOutput::new("test")
        .with_details(vec!["d1".to_string()]);
    let cloned = original.clone();
    assert_eq!(cloned.message, "test");
    assert_eq!(cloned.details.as_ref().unwrap().len(), 1);
}

// ============================================================================
// KeyValueOutput Tests
// ============================================================================

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
fn test_key_value_output_deserialize() {
    let json = r#"{"items":[{"key":"test_key","value":"test_value"}]}"#;
    let output: KeyValueOutput = serde_json::from_str(json).unwrap();
    assert_eq!(output.items.len(), 1);
    assert_eq!(output.items[0].key, "test_key");
}

#[test]
fn test_key_value_output_clone() {
    let original = KeyValueOutput::new()
        .add("k", "v");
    let cloned = original.clone();
    assert_eq!(cloned.items.len(), 1);
}

// ============================================================================
// KeyValueItem Tests
// ============================================================================

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
fn test_key_value_item_clone() {
    let original = KeyValueItem {
        key: "k".to_string(),
        value: "v".to_string(),
    };
    let cloned = original.clone();
    assert_eq!(cloned.key, "k");
    assert_eq!(cloned.value, "v");
}

// ============================================================================
// TableOutput Tests
// ============================================================================

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
fn test_table_output_serialize() {
    let output = TableOutput::new(vec![1, 2, 3]);
    let json = serde_json::to_string(&output).unwrap();
    assert_eq!(json, r#"{"rows":[1,2,3]}"#);
}

#[test]
fn test_table_output_deserialize() {
    let json = r#"{"rows":["a","b","c"]}"#;
    let output: TableOutput<String> = serde_json::from_str(json).unwrap();
    assert_eq!(output.rows.len(), 3);
    assert_eq!(output.rows[0], "a");
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

#[test]
fn test_table_output_clone() {
    let original = TableOutput::new(vec![1, 2, 3]);
    let cloned = original.clone();
    assert_eq!(cloned.rows, vec![1, 2, 3]);
}
