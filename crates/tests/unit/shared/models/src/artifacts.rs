//! Unit tests for artifact type models
//!
//! Tests cover:
//! - ArtifactType enum variants and serialization
//! - ColumnType enum variants
//! - ChartType enum variants and defaults
//! - AxisType enum variants and defaults
//! - SortOrder enum variants
//! - Alignment enum variants

use systemprompt_models::artifacts::SortOrder as ArtifactSortOrder;
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
fn test_artifact_type_copy_paste_text_serialize() {
    let json = serde_json::to_string(&ArtifactType::CopyPasteText).unwrap();
    assert_eq!(json, "\"copy_paste_text\"");
}

#[test]
fn test_artifact_type_custom_serialize() {
    let custom = ArtifactType::Custom("blog".to_string());
    let json = serde_json::to_string(&custom).unwrap();
    assert_eq!(json, "\"blog\"");
}

#[test]
fn test_artifact_type_text_deserialize() {
    let t: ArtifactType = serde_json::from_str("\"text\"").unwrap();
    assert!(matches!(t, ArtifactType::Text));
}

#[test]
fn test_artifact_type_table_deserialize() {
    let t: ArtifactType = serde_json::from_str("\"table\"").unwrap();
    assert!(matches!(t, ArtifactType::Table));
}

#[test]
fn test_artifact_type_chart_deserialize() {
    let t: ArtifactType = serde_json::from_str("\"chart\"").unwrap();
    assert!(matches!(t, ArtifactType::Chart));
}

#[test]
fn test_artifact_type_dashboard_deserialize() {
    let t: ArtifactType = serde_json::from_str("\"dashboard\"").unwrap();
    assert!(matches!(t, ArtifactType::Dashboard));
}

#[test]
fn test_artifact_type_custom_deserialize() {
    let t: ArtifactType = serde_json::from_str("\"custom_type\"").unwrap();
    match t {
        ArtifactType::Custom(s) => assert_eq!(s, "custom_type"),
        _ => panic!("Expected Custom variant"),
    }
}

#[test]
fn test_artifact_type_equality() {
    assert_eq!(ArtifactType::Text, ArtifactType::Text);
    assert_eq!(ArtifactType::Table, ArtifactType::Table);
    assert_ne!(ArtifactType::Text, ArtifactType::Table);
}

#[test]
fn test_artifact_type_custom_equality() {
    let c1 = ArtifactType::Custom("blog".to_string());
    let c2 = ArtifactType::Custom("blog".to_string());
    let c3 = ArtifactType::Custom("product".to_string());

    assert_eq!(c1, c2);
    assert_ne!(c1, c3);
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

#[test]
fn test_column_type_deserialize() {
    let t: ColumnType = serde_json::from_str("\"currency\"").unwrap();
    assert!(matches!(t, ColumnType::Currency));
}

#[test]
fn test_column_type_equality() {
    assert_eq!(ColumnType::String, ColumnType::String);
    assert_eq!(ColumnType::Integer, ColumnType::Integer);
    assert_ne!(ColumnType::String, ColumnType::Integer);
}

#[test]
fn test_column_type_copy() {
    let t = ColumnType::Number;
    let copied = t;
    assert_eq!(t, copied);
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

#[test]
fn test_chart_type_deserialize() {
    let t: ChartType = serde_json::from_str("\"bar\"").unwrap();
    assert!(matches!(t, ChartType::Bar));
}

#[test]
fn test_chart_type_equality() {
    assert_eq!(ChartType::Line, ChartType::Line);
    assert_eq!(ChartType::Bar, ChartType::Bar);
    assert_ne!(ChartType::Line, ChartType::Bar);
}

#[test]
fn test_chart_type_copy() {
    let t = ChartType::Pie;
    let copied = t;
    assert_eq!(t, copied);
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

#[test]
fn test_axis_type_deserialize() {
    let t: AxisType = serde_json::from_str("\"logarithmic\"").unwrap();
    assert!(matches!(t, AxisType::Logarithmic));
}

#[test]
fn test_axis_type_equality() {
    assert_eq!(AxisType::Linear, AxisType::Linear);
    assert_eq!(AxisType::Time, AxisType::Time);
    assert_ne!(AxisType::Linear, AxisType::Time);
}

#[test]
fn test_axis_type_copy() {
    let t = AxisType::Category;
    let copied = t;
    assert_eq!(t, copied);
}

// ============================================================================
// SortOrder Tests
// ============================================================================

#[test]
fn test_sort_order_asc_serialize() {
    let json = serde_json::to_string(&ArtifactSortOrder::Asc).unwrap();
    assert_eq!(json, "\"asc\"");
}

#[test]
fn test_sort_order_desc_serialize() {
    let json = serde_json::to_string(&ArtifactSortOrder::Desc).unwrap();
    assert_eq!(json, "\"desc\"");
}

#[test]
fn test_sort_order_deserialize_asc() {
    let order: ArtifactSortOrder = serde_json::from_str("\"asc\"").unwrap();
    assert!(matches!(order, ArtifactSortOrder::Asc));
}

#[test]
fn test_sort_order_deserialize_desc() {
    let order: ArtifactSortOrder = serde_json::from_str("\"desc\"").unwrap();
    assert!(matches!(order, ArtifactSortOrder::Desc));
}

#[test]
fn test_sort_order_equality() {
    assert_eq!(ArtifactSortOrder::Asc, ArtifactSortOrder::Asc);
    assert_eq!(ArtifactSortOrder::Desc, ArtifactSortOrder::Desc);
    assert_ne!(ArtifactSortOrder::Asc, ArtifactSortOrder::Desc);
}

#[test]
fn test_sort_order_copy() {
    let order = ArtifactSortOrder::Asc;
    let copied = order;
    assert_eq!(order, copied);
}

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

#[test]
fn test_alignment_deserialize_left() {
    let align: Alignment = serde_json::from_str("\"left\"").unwrap();
    assert!(matches!(align, Alignment::Left));
}

#[test]
fn test_alignment_deserialize_center() {
    let align: Alignment = serde_json::from_str("\"center\"").unwrap();
    assert!(matches!(align, Alignment::Center));
}

#[test]
fn test_alignment_deserialize_right() {
    let align: Alignment = serde_json::from_str("\"right\"").unwrap();
    assert!(matches!(align, Alignment::Right));
}

#[test]
fn test_alignment_equality() {
    assert_eq!(Alignment::Left, Alignment::Left);
    assert_eq!(Alignment::Center, Alignment::Center);
    assert_eq!(Alignment::Right, Alignment::Right);
    assert_ne!(Alignment::Left, Alignment::Right);
}

#[test]
fn test_alignment_copy() {
    let align = Alignment::Center;
    let copied = align;
    assert_eq!(align, copied);
}
