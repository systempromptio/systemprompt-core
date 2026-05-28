//! Tests for `render_result` and `truncate_with_ellipsis` re-export.
//!
//! `render_result` writes to stdout via the logging crate's CLI sink. We
//! cannot easily capture that here, but we can confirm every branch is
//! exercised without panicking — including the skip-render short-circuit
//! and each of the three OutputFormat arms plus the RawText special case.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde::Serialize;
use systemprompt_cli::cli_settings::{CliConfig, OutputFormat, set_global_config};
use systemprompt_cli::shared::{
    ArtifactType, ChartType, CommandResult, KeyValueOutput, RenderingHints, SuccessOutput,
    TableOutput, TextOutput, render_result,
};

#[derive(Debug, Clone, Serialize)]
struct Row {
    name: String,
    count: u32,
}

#[test]
fn render_result_skip_render_is_noop() {
    let r = CommandResult::table(TextOutput::new("ignored")).with_skip_render();
    render_result(&r);
}

#[test]
fn render_result_table_format_with_title() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Table));
    let r = CommandResult::table(TableOutput::new(vec![Row {
        name: "a".into(),
        count: 1,
    }]))
    .with_title("Things");
    render_result(&r);
}

#[test]
fn render_result_table_format_without_title() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Table));
    let r = CommandResult::table(KeyValueOutput::new().add("k", "v"));
    render_result(&r);
}

#[test]
fn render_result_json_format() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Json));
    let r = CommandResult::card(SuccessOutput::new("done")).with_title("Result");
    render_result(&r);
}

#[test]
fn render_result_yaml_format() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Yaml));
    let r = CommandResult::list(TextOutput::new("hello"));
    render_result(&r);
}

#[test]
fn render_result_raw_text_in_table_mode_with_title() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Table));
    let r = CommandResult::raw_text("plain text content".to_string()).with_title("Raw");
    render_result(&r);
}

#[test]
fn render_result_raw_text_in_json_mode() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Json));
    let r = CommandResult::raw_text("plain text".to_string());
    render_result(&r);
}

#[test]
fn render_result_chart_form_dashboard() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Json));
    render_result(&CommandResult::chart(vec![1_u32, 2, 3], ChartType::Bar));
    render_result(&CommandResult::form(TextOutput::new("f")));
    render_result(&CommandResult::dashboard(TextOutput::new("d")));
    render_result(&CommandResult::copy_paste(TextOutput::new("c")));
    render_result(&CommandResult::text(TextOutput::new("t")));
}

#[test]
fn rendering_hints_with_columns_and_extra() {
    let mut hints = RenderingHints::default();
    hints.columns = Some(vec!["a".to_string(), "b".to_string()]);
    hints.theme = Some("dark".to_string());
    hints
        .extra
        .insert("density".to_string(), serde_json::json!("compact"));
    let r = CommandResult::table(TextOutput::new("x")).with_hints(hints);
    let json = serde_json::to_string(&r).expect("serializes");
    assert!(json.contains("columns"));
    assert!(json.contains("density"));
}

#[test]
fn command_result_artifact_types_serialize() {
    for at in [
        ArtifactType::Table,
        ArtifactType::List,
        ArtifactType::PresentationCard,
        ArtifactType::Text,
        ArtifactType::CopyPasteText,
        ArtifactType::RawText,
        ArtifactType::Chart,
        ArtifactType::Form,
        ArtifactType::Dashboard,
    ] {
        let s = serde_json::to_string(&at).expect("serializes");
        assert!(s.starts_with('"'));
    }
}

#[test]
fn truncate_with_ellipsis_short_string_returned_as_is() {
    let out = systemprompt_cli::shared::truncate_with_ellipsis("hello", 10);
    assert_eq!(out, "hello");
}

#[test]
fn truncate_with_ellipsis_long_string_gets_truncated() {
    let out = systemprompt_cli::shared::truncate_with_ellipsis("hello world", 8);
    assert!(out.len() <= 8 || out.ends_with('…') || out.ends_with("..."));
    assert_ne!(out, "hello world");
}
