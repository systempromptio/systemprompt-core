#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::shared::{
    ArtifactType, ChartType, CommandResult, RenderingHints,
};

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
    let hints = result.hints.expect("chart should have hints");
    assert!(matches!(hints.chart_type, Some(ChartType::Bar)));
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
    let result_hints = result.hints.expect("should have hints after with_hints");
    assert_eq!(result_hints.theme.as_ref().unwrap(), "dark");
}

#[test]
fn test_command_result_with_columns() {
    let result = CommandResult::table("data")
        .with_columns(vec!["col1".to_string(), "col2".to_string()]);
    let result_hints = result.hints.expect("should have hints after with_columns");
    let columns = result_hints.columns.as_ref().unwrap();
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

    let final_hints = result.hints.expect("should have hints after chaining");
    assert_eq!(final_hints.theme.as_ref().unwrap(), "dark");
    final_hints.columns.expect("should have columns after with_columns");
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
    result.hints.expect("chart with builder chain should have hints");
}

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
