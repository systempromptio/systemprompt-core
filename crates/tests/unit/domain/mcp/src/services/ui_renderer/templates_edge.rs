// Column-inference fallbacks in the table renderer and the axis-scale /
// value-formatting branches in the chart SVG builder. Both sets of internals
// are `pub(super)` and only reachable through `UiRenderer::render`.

use systemprompt_mcp::services::ui_renderer::UiRenderer;
use systemprompt_mcp::services::ui_renderer::templates::{ChartRenderer, TableRenderer};
use systemprompt_models::artifacts::chart::{ChartArtifact, ChartDataset};
use systemprompt_models::artifacts::types::ChartType;
use systemprompt_models::{A2aArtifact as Artifact, ArtifactMetadata, DataPart, Part};

fn data_part(data: serde_json::Value) -> Part {
    Part::Data(DataPart {
        data: data.as_object().cloned().expect("object payload"),
    })
}

fn artifact(artifact_type: &str, parts: Vec<Part>) -> Artifact {
    let metadata = ArtifactMetadata::new(
        artifact_type.to_owned(),
        systemprompt_identifiers::ContextId::generate(),
        systemprompt_identifiers::TaskId::generate(),
    );
    Artifact {
        id: systemprompt_identifiers::ArtifactId::generate(),
        title: None,
        description: None,
        parts,
        extensions: vec![],
        metadata,
    }
}

fn chart_of(values: Vec<f64>) -> Artifact {
    let chart = ChartArtifact::new("Scaled", ChartType::Bar)
        .with_labels(values.iter().map(|v| format!("L{v}")).collect())
        .with_datasets(vec![ChartDataset::new("D", values)]);
    artifact(
        "chart",
        vec![data_part(serde_json::to_value(&chart).expect("chart json"))],
    )
}

#[tokio::test]
async fn table_infers_its_columns_from_the_first_row_object() {
    let result = TableRenderer::new()
        .render(&artifact(
            "table",
            vec![data_part(serde_json::json!({
                "data": [
                    {"region": "EU", "hits": 4},
                    {"region": "US", "hits": 9}
                ]
            }))],
        ))
        .await
        .expect("render");

    assert!(
        result.html.contains("region") && result.html.contains("hits"),
        "undeclared columns are taken from the first row's keys: {}",
        result.html
    );
    assert!(result.html.contains("EU") && result.html.contains("US"));
}

#[tokio::test]
async fn table_falls_back_to_positional_column_names_for_array_rows() {
    let result = TableRenderer::new()
        .render(&artifact(
            "table",
            vec![data_part(serde_json::json!({
                "data": [["a", "b", "c"], ["d", "e", "f"]]
            }))],
        ))
        .await
        .expect("render");

    assert!(
        result.html.contains("Column 1")
            && result.html.contains("Column 2")
            && result.html.contains("Column 3"),
        "rows without keys get positional headers: {}",
        result.html
    );
}

#[tokio::test]
async fn table_row_objects_missing_a_declared_column_render_an_empty_cell() {
    let result = TableRenderer::new()
        .render(&artifact(
            "table",
            vec![data_part(serde_json::json!({
                "columns": ["a", "b"],
                "data": [{"a": 1}]
            }))],
        ))
        .await
        .expect("render");

    assert!(
        result.html.contains('b'),
        "the declared column survives even with no data behind it"
    );
}

#[tokio::test]
async fn chart_axis_ticks_are_rounded_to_a_readable_step() {
    let result = ChartRenderer::new()
        .render(&chart_of(vec![0.0, 37.0, 84.0]))
        .await
        .expect("render");

    assert!(
        result.html.contains(">100<") || result.html.contains(">90<"),
        "the top tick is rounded up past the data maximum: {}",
        result.html
    );
}

#[tokio::test]
async fn chart_axis_ticks_render_large_magnitudes_without_decimals() {
    let result = ChartRenderer::new()
        .render(&chart_of(vec![0.0, 250_000.0]))
        .await
        .expect("render");

    assert!(
        !result.html.contains(".00<"),
        "values at or above 10,000 are printed whole: {}",
        result.html
    );
}

#[tokio::test]
async fn chart_axis_ticks_keep_significant_decimals_for_small_ranges() {
    let result = ChartRenderer::new()
        .render(&chart_of(vec![0.0, 0.05]))
        .await
        .expect("render");

    assert!(
        result.html.contains('.'),
        "a sub-unit range needs fractional ticks: {}",
        result.html
    );
    assert!(
        !result.html.contains("0.00<"),
        "trailing zeros are trimmed rather than padded: {}",
        result.html
    );
}

#[tokio::test]
async fn chart_with_a_flat_series_still_renders_a_unit_axis() {
    let result = ChartRenderer::new()
        .render(&chart_of(vec![7.0, 7.0, 7.0]))
        .await
        .expect("render");

    assert!(
        result.html.contains("chart-bar"),
        "a zero-range series falls back to a 0..1 axis rather than dividing by zero: {}",
        result.html
    );
}

#[tokio::test]
async fn chart_spanning_zero_renders_both_signs() {
    let result = ChartRenderer::new()
        .render(&chart_of(vec![-40.0, 0.0, 60.0]))
        .await
        .expect("render");

    assert!(
        result.html.contains('-'),
        "a negative minimum produces a negative axis tick: {}",
        result.html
    );
}
