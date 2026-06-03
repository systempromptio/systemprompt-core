#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::shared::{ChartType, CommandOutput};
use systemprompt_models::artifacts::{
    ChartArtifact, CliArtifact, DashboardArtifact, ListItem, NoticeLine, PresentationCardArtifact,
};

#[test]
fn test_command_output_table() {
    let output = CommandOutput::table(vec!["col"], vec![serde_json::json!({"col": "v"})]);
    assert!(matches!(output.artifact(), CliArtifact::Table { .. }));
    assert!(!output.should_skip_render());
}

#[test]
fn test_command_output_list() {
    let output = CommandOutput::list(vec![ListItem::new("title", "summary", "link")]);
    assert!(matches!(output.artifact(), CliArtifact::List { .. }));
}

#[test]
fn test_command_output_card() {
    let output = CommandOutput::card(PresentationCardArtifact::new("card content"));
    assert!(matches!(
        output.artifact(),
        CliArtifact::PresentationCard { .. }
    ));
}

#[test]
fn test_command_output_card_value() {
    let output = CommandOutput::card_value("title", &serde_json::json!({"k": "v"}));
    assert!(matches!(
        output.artifact(),
        CliArtifact::PresentationCard { .. }
    ));
}

#[test]
fn test_command_output_text() {
    let output = CommandOutput::text("plain text");
    assert!(matches!(output.artifact(), CliArtifact::Text { .. }));
}

#[test]
fn test_command_output_text_titled() {
    let output = CommandOutput::text_titled("Greeting", "hello");
    assert!(matches!(output.artifact(), CliArtifact::Text { .. }));
}

#[test]
fn test_command_output_copy_paste() {
    let output = CommandOutput::copy_paste("copyable text");
    assert!(matches!(
        output.artifact(),
        CliArtifact::CopyPasteText { .. }
    ));
}

#[test]
fn test_command_output_copy_paste_titled() {
    let output = CommandOutput::copy_paste_titled("Title", "copyable text");
    assert!(matches!(
        output.artifact(),
        CliArtifact::CopyPasteText { .. }
    ));
}

#[test]
fn test_command_output_chart() {
    let output = CommandOutput::chart(ChartArtifact::new("Sales", ChartType::Bar));
    assert!(matches!(output.artifact(), CliArtifact::Chart { .. }));
}

#[test]
fn test_command_output_dashboard() {
    let output = CommandOutput::dashboard(DashboardArtifact::new("dashboard"));
    assert!(matches!(output.artifact(), CliArtifact::Dashboard { .. }));
}

#[test]
fn test_command_output_with_title() {
    let output = CommandOutput::text("data").with_title("My Title");
    // Title is terminal-only presentation state; the artifact variant is unchanged.
    assert!(matches!(output.artifact(), CliArtifact::Text { .. }));
}

#[test]
fn test_command_output_with_title_into_string() {
    let output = CommandOutput::text("data").with_title(String::from("String Title"));
    assert!(matches!(output.artifact(), CliArtifact::Text { .. }));
}

#[test]
fn test_command_output_skip_render() {
    let output = CommandOutput::text("data");
    assert!(!output.should_skip_render());

    let output = output.with_skip_render();
    assert!(output.should_skip_render());
}

#[test]
fn test_command_output_builder_chain() {
    let output = CommandOutput::chart(ChartArtifact::new("Sales Data", ChartType::Line))
        .with_title("Sales Data")
        .with_skip_render();

    assert!(matches!(output.artifact(), CliArtifact::Chart { .. }));
    assert!(output.should_skip_render());
}

#[test]
fn test_command_output_table_of() {
    #[derive(serde::Serialize)]
    struct Row {
        name: String,
        value: u32,
    }
    let rows = vec![Row {
        name: "a".into(),
        value: 1,
    }];
    let output = CommandOutput::table_of(vec!["name", "value"], &rows);
    assert!(matches!(output.artifact(), CliArtifact::Table { .. }));
}

#[test]
fn test_command_output_serialize_artifact() {
    let output = CommandOutput::text("hello");
    let json = serde_json::to_string(output.artifact()).unwrap();
    // The wire form is the tagged CliArtifact union.
    assert!(json.contains("\"artifact_type\":\"text\""));
    assert!(json.contains("hello"));
}

#[test]
fn test_command_output_from_cli_artifact() {
    let artifact = CliArtifact::text(systemprompt_models::artifacts::TextArtifact::new("x"));
    let output: CommandOutput = artifact.into();
    assert!(matches!(output.artifact(), CliArtifact::Text { .. }));
}

#[test]
fn test_command_output_message() {
    let output = CommandOutput::message(vec![
        NoticeLine::new("warning", "no rows"),
        NoticeLine::new("info", "tip"),
    ]);
    assert!(matches!(output.artifact(), CliArtifact::Message { .. }));
}

#[test]
fn test_command_output_message_serializes() {
    let output = CommandOutput::message(vec![NoticeLine::new("error", "boom")]);
    let json = serde_json::to_string(output.artifact()).unwrap();
    assert!(json.contains("\"artifact_type\":\"message\""));
    assert!(json.contains("boom"));
}
