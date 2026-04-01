//! Unit tests for artifact type models
//!
//! Tests cover:
//! - ArtifactType enum variants and serialization
//! - ColumnType enum variants
//! - ChartType enum variants and defaults
//! - AxisType enum variants and defaults
//! - SortOrder enum variants
//! - Alignment enum variants

use systemprompt_models::{Alignment, ArtifactType, AxisType, ChartType, ColumnType};

// ============================================================================
// ArtifactType Tests
// ============================================================================

#[test]
fn test_artifact_type_text_serialize() {
    let json = serde_json::to_string(&ArtifactType::Text).unwrap();
    assert_eq!(json, "\"text\"");
}

#[test]
fn test_artifact_type_table_serialize() {
    let json = serde_json::to_string(&ArtifactType::Table).unwrap();
    assert_eq!(json, "\"table\"");
}

#[test]
fn test_artifact_type_chart_serialize() {
    let json = serde_json::to_string(&ArtifactType::Chart).unwrap();
    assert_eq!(json, "\"chart\"");
}

#[test]
fn test_artifact_type_form_serialize() {
    let json = serde_json::to_string(&ArtifactType::Form).unwrap();
    assert_eq!(json, "\"form\"");
}

#[test]
fn test_artifact_type_dashboard_serialize() {
    let json = serde_json::to_string(&ArtifactType::Dashboard).unwrap();
    assert_eq!(json, "\"dashboard\"");
}

#[test]
fn test_artifact_type_presentation_card_serialize() {
    let json = serde_json::to_string(&ArtifactType::PresentationCard).unwrap();
    assert_eq!(json, "\"presentation_card\"");
}

#[test]
fn test_artifact_type_list_serialize() {
    let json = serde_json::to_string(&ArtifactType::List).unwrap();
    assert_eq!(json, "\"list\"");
}

#[test]
fn test_artifact_type_custom_serialize() {
    let custom = ArtifactType::Custom("blog".to_string());
    let json = serde_json::to_string(&custom).unwrap();
    assert_eq!(json, "\"blog\"");
}

#[test]
fn test_artifact_type_custom_deserialize() {
    let t: ArtifactType = serde_json::from_str("\"custom_type\"").unwrap();
    match t {
        ArtifactType::Custom(s) => assert_eq!(s, "custom_type"),
        _ => panic!("Expected Custom variant"),
    }
}

// ============================================================================
// ColumnType Tests
// ============================================================================

#[test]
fn test_column_type_string_serialize() {
    let json = serde_json::to_string(&ColumnType::String).unwrap();
    assert_eq!(json, "\"string\"");
}

#[test]
fn test_column_type_integer_serialize() {
    let json = serde_json::to_string(&ColumnType::Integer).unwrap();
    assert_eq!(json, "\"integer\"");
}

#[test]
fn test_column_type_number_serialize() {
    let json = serde_json::to_string(&ColumnType::Number).unwrap();
    assert_eq!(json, "\"number\"");
}

#[test]
fn test_column_type_currency_serialize() {
    let json = serde_json::to_string(&ColumnType::Currency).unwrap();
    assert_eq!(json, "\"currency\"");
}

#[test]
fn test_column_type_percentage_serialize() {
    let json = serde_json::to_string(&ColumnType::Percentage).unwrap();
    assert_eq!(json, "\"percentage\"");
}

#[test]
fn test_column_type_date_serialize() {
    let json = serde_json::to_string(&ColumnType::Date).unwrap();
    assert_eq!(json, "\"date\"");
}

#[test]
fn test_column_type_boolean_serialize() {
    let json = serde_json::to_string(&ColumnType::Boolean).unwrap();
    assert_eq!(json, "\"boolean\"");
}

#[test]
fn test_column_type_link_serialize() {
    let json = serde_json::to_string(&ColumnType::Link).unwrap();
    assert_eq!(json, "\"link\"");
}

// ============================================================================
// ChartType Tests
// ============================================================================

#[test]
fn test_chart_type_default_is_line() {
    let default = ChartType::default();
    assert!(matches!(default, ChartType::Line));
}

#[test]
fn test_chart_type_line_serialize() {
    let json = serde_json::to_string(&ChartType::Line).unwrap();
    assert_eq!(json, "\"line\"");
}

#[test]
fn test_chart_type_bar_serialize() {
    let json = serde_json::to_string(&ChartType::Bar).unwrap();
    assert_eq!(json, "\"bar\"");
}

#[test]
fn test_chart_type_pie_serialize() {
    let json = serde_json::to_string(&ChartType::Pie).unwrap();
    assert_eq!(json, "\"pie\"");
}

#[test]
fn test_chart_type_doughnut_serialize() {
    let json = serde_json::to_string(&ChartType::Doughnut).unwrap();
    assert_eq!(json, "\"doughnut\"");
}

#[test]
fn test_chart_type_area_serialize() {
    let json = serde_json::to_string(&ChartType::Area).unwrap();
    assert_eq!(json, "\"area\"");
}

// ============================================================================
// AxisType Tests
// ============================================================================

#[test]
fn test_axis_type_default_is_linear() {
    let default = AxisType::default();
    assert!(matches!(default, AxisType::Linear));
}

#[test]
fn test_axis_type_category_serialize() {
    let json = serde_json::to_string(&AxisType::Category).unwrap();
    assert_eq!(json, "\"category\"");
}

#[test]
fn test_axis_type_linear_serialize() {
    let json = serde_json::to_string(&AxisType::Linear).unwrap();
    assert_eq!(json, "\"linear\"");
}

#[test]
fn test_axis_type_logarithmic_serialize() {
    let json = serde_json::to_string(&AxisType::Logarithmic).unwrap();
    assert_eq!(json, "\"logarithmic\"");
}

#[test]
fn test_axis_type_time_serialize() {
    let json = serde_json::to_string(&AxisType::Time).unwrap();
    assert_eq!(json, "\"time\"");
}

// ============================================================================
// SortOrder Tests
// ============================================================================

// ============================================================================
// Alignment Tests
// ============================================================================

#[test]
fn test_alignment_left_serialize() {
    let json = serde_json::to_string(&Alignment::Left).unwrap();
    assert_eq!(json, "\"left\"");
}

#[test]
fn test_alignment_center_serialize() {
    let json = serde_json::to_string(&Alignment::Center).unwrap();
    assert_eq!(json, "\"center\"");
}

#[test]
fn test_alignment_right_serialize() {
    let json = serde_json::to_string(&Alignment::Right).unwrap();
    assert_eq!(json, "\"right\"");
}
