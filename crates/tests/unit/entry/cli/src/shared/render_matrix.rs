//! Tests that drive `render_result` over every artifact kind in every output
//! format.
//!
//! The terminal renderer has a distinct arm per artifact kind, and the JSON and
//! YAML paths bypass it entirely; only a matrix reaches all of them.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::shared::{CommandOutput, render_result};
use systemprompt_cli::{CliConfig, OutputFormat};
use systemprompt_models::artifacts::{
    ChartArtifact, ChartDataset, ChartType, DashboardArtifact, ListItem, NoticeLine,
    PresentationCardArtifact,
};

fn configs() -> Vec<CliConfig> {
    vec![
        CliConfig::new().with_output_format(OutputFormat::Json),
        CliConfig::new().with_output_format(OutputFormat::Yaml),
        CliConfig::new().with_output_format(OutputFormat::Table),
    ]
}

fn render_in_every_format(output: &CommandOutput) {
    for config in configs() {
        render_result(output, &config);
    }
}

#[test]
fn text_and_copy_paste_artifacts_render_titled_and_untitled() {
    render_in_every_format(&CommandOutput::text("plain body"));
    render_in_every_format(&CommandOutput::text_titled("Titled", "body"));
    render_in_every_format(&CommandOutput::copy_paste("copy me"));
    render_in_every_format(&CommandOutput::copy_paste_titled("Snippet", "copy me"));

    // An outer title takes precedence over the artifact's own.
    render_in_every_format(&CommandOutput::text_titled("Inner", "body").with_title("Outer"));
}

#[test]
fn table_artifacts_render_with_and_without_rows() {
    let empty = CommandOutput::table(vec!["a", "b"], vec![]).with_title("Empty");
    render_in_every_format(&empty);

    let populated = CommandOutput::table(
        vec!["name", "count"],
        vec![
            serde_json::json!({"name": "alpha", "count": 1}),
            serde_json::json!({"name": "beta", "count": 2}),
        ],
    )
    .with_title("Rows");
    render_in_every_format(&populated);
}

#[test]
fn list_and_card_artifacts_render() {
    let list = CommandOutput::list(vec![
        ListItem::new("first", "the first item", "/first"),
        ListItem::new("second", "the second item", "/second"),
    ]);
    render_in_every_format(&list);

    let card = CommandOutput::card_value(
        "Card",
        &serde_json::json!({"key": "value", "nested": {"inner": 1}}),
    );
    render_in_every_format(&card);

    // A non-object value becomes a single section.
    render_in_every_format(&CommandOutput::card_value("Scalar", &"just a string"));
    render_in_every_format(&CommandOutput::card(PresentationCardArtifact::new(
        "Bare card",
    )));
}

#[test]
fn chart_and_dashboard_artifacts_render() {
    let chart = ChartArtifact::new("Visits", ChartType::Line)
        .with_labels(vec!["mon".to_owned(), "tue".to_owned()])
        .with_datasets(vec![ChartDataset::new("visits", vec![1.0, 2.0])]);
    render_in_every_format(&CommandOutput::chart(chart));

    let dashboard = DashboardArtifact::new("Dashboard");
    render_in_every_format(&CommandOutput::dashboard(dashboard));
}

#[test]
fn message_artifacts_render_each_severity() {
    let message = CommandOutput::message(vec![
        NoticeLine::new("success", "it worked"),
        NoticeLine::new("warning", "be careful"),
        NoticeLine::new("error", "it broke"),
        NoticeLine::new("info", "for your information"),
    ]);
    render_in_every_format(&message);
}

#[test]
fn a_skip_render_output_produces_nothing_in_any_format() {
    let skipped = CommandOutput::text("never shown").with_skip_render();
    assert!(skipped.should_skip_render());
    render_in_every_format(&skipped);
}

#[test]
fn an_artifact_survives_the_round_trip_through_into_artifact() {
    let output = CommandOutput::text_titled("Round trip", "body");
    assert_eq!(output.title(), Some("Round trip"));

    let artifact = output.into_artifact();
    let rebuilt = CommandOutput::from(artifact);
    assert!(!rebuilt.should_skip_render());
    render_in_every_format(&rebuilt);
}
