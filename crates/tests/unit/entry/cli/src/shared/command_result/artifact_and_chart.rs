#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::shared::{
    ArtifactType, ChartType, RenderingHints,
};

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
