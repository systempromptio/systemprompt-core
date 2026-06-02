//! Tests for `render_result` and `truncate_with_ellipsis` re-export.
//!
//! `render_result` writes to stdout via the logging crate's CLI sink. We
//! cannot easily capture that here, but we can confirm every branch is
//! exercised without panicking — including the skip-render short-circuit
//! and each of the three OutputFormat arms.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde::Serialize;
use systemprompt_cli::cli_settings::{CliConfig, OutputFormat, set_global_config};
use systemprompt_cli::shared::{ChartType, CommandOutput, render_result};
use systemprompt_models::artifacts::{
    ChartArtifact, DashboardArtifact, ListItem, PresentationCardArtifact,
};

#[derive(Debug, Clone, Serialize)]
struct Row {
    name: String,
    count: u32,
}

#[test]
fn render_result_skip_render_is_noop() {
    let r = CommandOutput::text("ignored").with_skip_render();
    render_result(&r);
}

#[test]
fn render_result_table_format_with_title() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Table));
    let rows = vec![Row {
        name: "a".into(),
        count: 1,
    }];
    let r = CommandOutput::table_of(vec!["name", "count"], &rows).with_title("Things");
    render_result(&r);
}

#[test]
fn render_result_table_format_without_title() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Table));
    let r = CommandOutput::table(vec!["k"], vec![serde_json::json!({"k": "v"})]);
    render_result(&r);
}

#[test]
fn render_result_json_format() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Json));
    let r = CommandOutput::card(PresentationCardArtifact::new("done")).with_title("Result");
    render_result(&r);
}

#[test]
fn render_result_yaml_format() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Yaml));
    let r = CommandOutput::list(vec![ListItem::new("hello", "", "")]);
    render_result(&r);
}

#[test]
fn render_result_text_in_table_mode_with_title() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Table));
    let r = CommandOutput::text("plain text content").with_title("Raw");
    render_result(&r);
}

#[test]
fn render_result_text_in_json_mode() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Json));
    let r = CommandOutput::text("plain text");
    render_result(&r);
}

#[test]
fn render_result_chart_dashboard_copy_paste_text() {
    set_global_config(CliConfig::default().with_output_format(OutputFormat::Json));
    render_result(&CommandOutput::chart(ChartArtifact::new("c", ChartType::Bar)));
    render_result(&CommandOutput::dashboard(DashboardArtifact::new("d")));
    render_result(&CommandOutput::copy_paste("c"));
    render_result(&CommandOutput::text("t"));
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
