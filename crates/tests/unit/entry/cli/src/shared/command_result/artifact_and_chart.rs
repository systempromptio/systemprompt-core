#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::shared::ChartType;

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
