use serde_json::json;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::artifacts::chart::ChartDataset;
use systemprompt_models::artifacts::dashboard::{
    DashboardHints, DashboardSection, DatabaseStatus, ErrorCounts, ItemList, LayoutMode,
    LayoutWidth, ListItem as DashboardListItem, ListSectionData, MetricCard, MetricStatus,
    MetricsCardsData, SectionLayout, SectionType, ServiceStatus, StatusSectionData,
    TableSectionData,
};
use systemprompt_models::artifacts::research::{ResearchArtifact, SourceCitation};
use systemprompt_models::artifacts::table::{Column, TableHints};
use systemprompt_models::artifacts::types::SortOrder as ArtifactSortOrder;
use systemprompt_models::artifacts::traits::ArtifactSchema;
use systemprompt_models::{
    Alignment, ArtifactType, AxisType, ChartType, CliArtifact, ColumnType, RenderingHints,
    RequestContext,
};

fn test_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("test-session"),
        TraceId::new("test-trace"),
        ContextId::new("test-context"),
        AgentName::new("test-agent"),
    )
}

#[test]
fn column_new_basic() {
    let col = Column::new("name", ColumnType::String);
    assert_eq!(col.name(), "name");
    assert_eq!(col.column_type(), ColumnType::String);
    assert!(col.label.is_none());
    assert!(col.width.is_none());
    assert!(col.align.is_none());
}

#[test]
fn column_with_header() {
    let col = Column::new("email", ColumnType::String).with_header("Email Address");
    assert_eq!(col.label.as_deref(), Some("Email Address"));
}

#[test]
fn column_with_label_alias() {
    let col = Column::new("age", ColumnType::Integer).with_label("User Age");
    assert_eq!(col.label.as_deref(), Some("User Age"));
}

#[test]
fn column_with_width() {
    let col = Column::new("id", ColumnType::Integer).with_width(100);
    assert_eq!(col.width, Some(100));
}

#[test]
fn column_with_alignment() {
    let col = Column::new("amount", ColumnType::Currency).with_alignment(Alignment::Right);
    assert_eq!(col.align, Some(Alignment::Right));
}

#[test]
fn column_builder_chain() {
    let col = Column::new("price", ColumnType::Currency)
        .with_header("Price (USD)")
        .with_width(120)
        .with_alignment(Alignment::Right);
    assert_eq!(col.name(), "price");
    assert_eq!(col.column_type(), ColumnType::Currency);
    assert_eq!(col.label.as_deref(), Some("Price (USD)"));
    assert_eq!(col.width, Some(120));
    assert_eq!(col.align, Some(Alignment::Right));
}

#[test]
fn column_serde_roundtrip() {
    let col = Column::new("status", ColumnType::Boolean)
        .with_header("Active")
        .with_width(80)
        .with_alignment(Alignment::Center);
    let json = serde_json::to_string(&col).unwrap();
    let deserialized: Column = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name(), "status");
    assert_eq!(deserialized.column_type(), ColumnType::Boolean);
    assert_eq!(deserialized.label.as_deref(), Some("Active"));
}

#[test]
fn table_hints_default() {
    let hints = TableHints::new();
    assert!(hints.columns.is_empty());
    assert!(hints.sortable_columns.is_empty());
    assert!(hints.default_sort.is_none());
    assert!(!hints.filterable);
    assert!(hints.page_size.is_none());
    assert!(!hints.row_click_enabled);
}

#[test]
fn table_hints_with_columns() {
    let cols = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String),
    ];
    let hints = TableHints::new().with_columns(cols);
    assert_eq!(hints.columns.len(), 2);
}

#[test]
fn table_hints_with_sortable() {
    let hints =
        TableHints::new().with_sortable(vec!["name".to_string(), "created_at".to_string()]);
    assert_eq!(hints.sortable_columns.len(), 2);
}

#[test]
fn table_hints_with_default_sort() {
    let hints =
        TableHints::new().with_default_sort("created_at".to_string(), ArtifactSortOrder::Desc);
    let (col, order) = hints.default_sort.unwrap();
    assert_eq!(col, "created_at");
    assert!(matches!(order, ArtifactSortOrder::Desc));
}

#[test]
fn table_hints_filterable() {
    let hints = TableHints::new().filterable();
    assert!(hints.filterable);
}

#[test]
fn table_hints_with_page_size() {
    let hints = TableHints::new().with_page_size(25);
    assert_eq!(hints.page_size, Some(25));
}

#[test]
fn table_hints_row_click_enabled() {
    let hints = TableHints::new().with_row_click_enabled(true);
    assert!(hints.row_click_enabled);
}

#[test]
fn table_hints_generate_schema_basic() {
    let hints = TableHints::new()
        .with_columns(vec![Column::new("id", ColumnType::Integer)])
        .filterable();
    let schema = hints.generate_schema();
    assert_eq!(schema["filterable"], json!(true));
    assert_eq!(schema["columns"], json!(["id"]));
}

#[test]
fn table_hints_generate_schema_with_sort() {
    let hints =
        TableHints::new().with_default_sort("name".to_string(), ArtifactSortOrder::Asc);
    let schema = hints.generate_schema();
    assert!(schema.get("default_sort").is_some());
    assert_eq!(schema["default_sort"]["column"], json!("name"));
}

#[test]
fn table_hints_generate_schema_with_page_size() {
    let hints = TableHints::new().with_page_size(50);
    let schema = hints.generate_schema();
    assert_eq!(schema["page_size"], json!(50));
}

#[test]
fn table_hints_generate_schema_row_click() {
    let hints = TableHints::new().with_row_click_enabled(true);
    let schema = hints.generate_schema();
    assert_eq!(schema["row_click_enabled"], json!(true));
}

#[test]
fn dashboard_hints_default() {
    let hints = DashboardHints::new();
    assert!(matches!(hints.layout, LayoutMode::Vertical));
    assert!(!hints.refreshable);
    assert!(hints.refresh_interval_seconds.is_none());
    assert!(!hints.drill_down_enabled);
}

#[test]
fn dashboard_hints_with_refreshable() {
    let hints = DashboardHints::new().with_refreshable(true);
    assert!(hints.refreshable);
}

#[test]
fn dashboard_hints_with_refresh_interval() {
    let hints = DashboardHints::new().with_refresh_interval(30);
    assert_eq!(hints.refresh_interval_seconds, Some(30));
}

#[test]
fn dashboard_hints_with_drill_down() {
    let hints = DashboardHints::new().with_drill_down(true);
    assert!(hints.drill_down_enabled);
}

#[test]
fn dashboard_hints_with_layout_grid() {
    let hints = DashboardHints::new().with_layout(LayoutMode::Grid);
    assert!(matches!(hints.layout, LayoutMode::Grid));
}

#[test]
fn dashboard_hints_with_layout_tabs() {
    let hints = DashboardHints::new().with_layout(LayoutMode::Tabs);
    assert!(matches!(hints.layout, LayoutMode::Tabs));
}

#[test]
fn dashboard_hints_generate_schema() {
    let hints = DashboardHints::new()
        .with_refreshable(true)
        .with_refresh_interval(60);
    let schema = hints.generate_schema();
    assert_eq!(schema["refreshable"], json!(true));
    assert_eq!(schema["refresh_interval_seconds"], json!(60));
}

#[test]
fn dashboard_hints_generate_schema_no_interval() {
    let hints = DashboardHints::new();
    let schema = hints.generate_schema();
    assert!(schema.get("refresh_interval_seconds").is_none());
}

#[test]
fn dashboard_hints_serde_roundtrip() {
    let hints = DashboardHints::new()
        .with_layout(LayoutMode::Grid)
        .with_refreshable(true)
        .with_drill_down(true);
    let json = serde_json::to_string(&hints).unwrap();
    let deserialized: DashboardHints = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized.layout, LayoutMode::Grid));
    assert!(deserialized.refreshable);
    assert!(deserialized.drill_down_enabled);
}

#[test]
fn metric_card_new() {
    let card = MetricCard::new("CPU Usage", "85%");
    assert_eq!(card.title, "CPU Usage");
    assert_eq!(card.value, "85%");
    assert!(card.subtitle.is_none());
    assert!(card.icon.is_none());
    assert!(card.status.is_none());
}

#[test]
fn metric_card_with_subtitle() {
    let card = MetricCard::new("Memory", "4.2 GB").with_subtitle("of 8 GB total");
    assert_eq!(card.subtitle.as_deref(), Some("of 8 GB total"));
}

#[test]
fn metric_card_with_icon() {
    let card = MetricCard::new("Disk", "120 GB").with_icon("disk");
    assert_eq!(card.icon.as_deref(), Some("disk"));
}

#[test]
fn metric_card_with_status() {
    let card = MetricCard::new("Health", "OK").with_status(MetricStatus::Success);
    assert!(matches!(card.status, Some(MetricStatus::Success)));
}

#[test]
fn metric_card_full_builder() {
    let card = MetricCard::new("Errors", "12")
        .with_subtitle("last 24 hours")
        .with_icon("alert")
        .with_status(MetricStatus::Warning);
    assert_eq!(card.title, "Errors");
    assert_eq!(card.value, "12");
    assert!(card.subtitle.is_some());
    assert!(card.icon.is_some());
    assert!(matches!(card.status, Some(MetricStatus::Warning)));
}

#[test]
fn metric_card_serde_roundtrip() {
    let card = MetricCard::new("Uptime", "99.9%").with_status(MetricStatus::Success);
    let json = serde_json::to_string(&card).unwrap();
    let deserialized: MetricCard = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "Uptime");
    assert_eq!(deserialized.value, "99.9%");
}

#[test]
fn metric_status_default_is_info() {
    let status = MetricStatus::default();
    assert!(matches!(status, MetricStatus::Info));
}

#[test]
fn metric_status_from_str_success_variants() {
    assert!(matches!("success".parse::<MetricStatus>(), Ok(MetricStatus::Success)));
    assert!(matches!("healthy".parse::<MetricStatus>(), Ok(MetricStatus::Success)));
    assert!(matches!("ok".parse::<MetricStatus>(), Ok(MetricStatus::Success)));
    assert!(matches!("active".parse::<MetricStatus>(), Ok(MetricStatus::Success)));
}

#[test]
fn metric_status_from_str_warning_variants() {
    assert!(matches!("warning".parse::<MetricStatus>(), Ok(MetricStatus::Warning)));
    assert!(matches!("degraded".parse::<MetricStatus>(), Ok(MetricStatus::Warning)));
}

#[test]
fn metric_status_from_str_error_variants() {
    assert!(matches!("error".parse::<MetricStatus>(), Ok(MetricStatus::Error)));
    assert!(matches!("failed".parse::<MetricStatus>(), Ok(MetricStatus::Error)));
    assert!(matches!("critical".parse::<MetricStatus>(), Ok(MetricStatus::Error)));
}

#[test]
fn metric_status_from_str_info_variants() {
    assert!(matches!("info".parse::<MetricStatus>(), Ok(MetricStatus::Info)));
    assert!(matches!("unknown".parse::<MetricStatus>(), Ok(MetricStatus::Info)));
}

#[test]
fn metric_status_from_str_invalid() {
    assert!("invalid_status".parse::<MetricStatus>().is_err());
}

#[test]
fn metrics_cards_data_new() {
    let cards = vec![MetricCard::new("A", "1"), MetricCard::new("B", "2")];
    let data = MetricsCardsData::new(cards);
    assert_eq!(data.cards.len(), 2);
}

#[test]
fn metrics_cards_data_add_card() {
    let data = MetricsCardsData::new(vec![]).add_card(MetricCard::new("New", "100"));
    assert_eq!(data.cards.len(), 1);
}

#[test]
fn chart_dataset_new() {
    let dataset = ChartDataset::new("Revenue", vec![100.0, 200.0, 300.0]);
    assert_eq!(dataset.label, "Revenue");
    assert_eq!(dataset.data.len(), 3);
    assert_eq!(dataset.data[0], 100.0);
}

#[test]
fn chart_dataset_serde_roundtrip() {
    let dataset = ChartDataset::new("Sales", vec![10.5, 20.3]);
    let json = serde_json::to_string(&dataset).unwrap();
    let deserialized: ChartDataset = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.label, "Sales");
    assert_eq!(deserialized.data, vec![10.5, 20.3]);
}

#[test]
fn source_citation_new() {
    let citation = SourceCitation::new("Rust Book", "https://doc.rust-lang.org", 0.95);
    assert_eq!(citation.title, "Rust Book");
    assert_eq!(citation.uri, "https://doc.rust-lang.org");
    assert_eq!(citation.relevance, 0.95);
}

#[test]
fn source_citation_serde_roundtrip() {
    let citation = SourceCitation::new("Wikipedia", "https://en.wikipedia.org", 0.8);
    let json = serde_json::to_string(&citation).unwrap();
    let deserialized: SourceCitation = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.title, "Wikipedia");
    assert_eq!(deserialized.relevance, 0.8);
}

#[test]
fn rendering_hints_default() {
    let hints = RenderingHints::default();
    assert!(hints.columns.is_none());
    assert!(hints.chart_type.is_none());
    assert!(hints.theme.is_none());
    assert!(hints.extra.is_empty());
}

#[test]
fn rendering_hints_serde_roundtrip() {
    let json_str = r#"{"columns":["a","b"],"chart_type":"bar","theme":"dark"}"#;
    let hints: RenderingHints = serde_json::from_str(json_str).unwrap();
    assert_eq!(hints.columns.as_ref().unwrap().len(), 2);
    assert_eq!(hints.chart_type.as_deref(), Some("bar"));
    assert_eq!(hints.theme.as_deref(), Some("dark"));
}

#[test]
fn dashboard_section_new() {
    let section = DashboardSection::new("section-1", "Overview", SectionType::MetricsCards);
    assert_eq!(section.section_id, "section-1");
    assert_eq!(section.title, "Overview");
    assert!(matches!(section.section_type, SectionType::MetricsCards));
}

#[test]
fn dashboard_section_with_order() {
    let section =
        DashboardSection::new("s1", "Title", SectionType::Table).with_order(5);
    assert_eq!(section.layout.order, 5);
}

#[test]
fn dashboard_section_with_layout() {
    let layout = SectionLayout {
        width: LayoutWidth::Half,
        order: 2,
    };
    let section =
        DashboardSection::new("s1", "Title", SectionType::Chart).with_layout(layout);
    assert!(matches!(section.layout.width, LayoutWidth::Half));
    assert_eq!(section.layout.order, 2);
}

#[test]
fn section_layout_default() {
    let layout = SectionLayout::default();
    assert!(matches!(layout.width, LayoutWidth::Full));
    assert_eq!(layout.order, 0);
}

#[test]
fn service_status_new() {
    let status = ServiceStatus::new("api-server", "running");
    assert_eq!(status.name, "api-server");
    assert_eq!(status.status, "running");
    assert!(status.uptime.is_none());
}

#[test]
fn service_status_with_uptime() {
    let status = ServiceStatus::new("db", "healthy").with_uptime("48h 12m");
    assert_eq!(status.uptime.as_deref(), Some("48h 12m"));
}

#[test]
fn database_status_new() {
    let status = DatabaseStatus::new(256.5, "healthy");
    assert_eq!(status.size_mb, 256.5);
    assert_eq!(status.status, "healthy");
}

#[test]
fn error_counts_new() {
    let counts = ErrorCounts::new(2, 15, 100);
    assert_eq!(counts.critical, 2);
    assert_eq!(counts.error, 15);
    assert_eq!(counts.warn, 100);
}

#[test]
fn status_section_data_basic() {
    let data = StatusSectionData::new(vec![ServiceStatus::new("web", "running")]);
    assert_eq!(data.services.len(), 1);
    assert!(data.database.is_none());
    assert!(data.recent_errors.is_none());
}

#[test]
fn status_section_data_with_database() {
    let data =
        StatusSectionData::new(vec![]).with_database(DatabaseStatus::new(100.0, "ok"));
    assert!(data.database.is_some());
}

#[test]
fn status_section_data_with_error_counts() {
    let data = StatusSectionData::new(vec![]).with_error_counts(ErrorCounts::new(0, 3, 10));
    let counts = data.recent_errors.unwrap();
    assert_eq!(counts.error, 3);
}

#[test]
fn table_section_data_basic() {
    let data = TableSectionData::new(
        vec!["name".to_string(), "value".to_string()],
        vec![json!({"name": "test", "value": 42})],
    );
    assert_eq!(data.columns.len(), 2);
    assert_eq!(data.rows.len(), 1);
    assert!(data.sortable.is_none());
}

#[test]
fn table_section_data_with_sortable() {
    let data = TableSectionData::new(vec![], vec![]).with_sortable(true);
    assert_eq!(data.sortable, Some(true));
}

#[test]
fn table_section_data_with_default_sort() {
    let data =
        TableSectionData::new(vec![], vec![]).with_default_sort("name", "asc");
    let sort = data.default_sort.unwrap();
    assert_eq!(sort.column, "name");
    assert_eq!(sort.order, "asc");
}

#[test]
fn list_section_data_new() {
    let data = ListSectionData::new(vec![]);
    assert!(data.lists.is_empty());
}

#[test]
fn item_list_new() {
    let items = vec![DashboardListItem::new(1, "First", "100")];
    let list = ItemList::new("Top Items", items);
    assert_eq!(list.title, "Top Items");
    assert_eq!(list.items.len(), 1);
}

#[test]
fn dashboard_list_item_new() {
    let item = DashboardListItem::new(1, "Page A", "500 views");
    assert_eq!(item.rank, 1);
    assert_eq!(item.label, "Page A");
    assert_eq!(item.value, "500 views");
    assert!(item.badge.is_none());
}

#[test]
fn dashboard_list_item_with_badge() {
    let item = DashboardListItem::new(1, "Top", "999").with_badge("hot");
    assert_eq!(item.badge.as_deref(), Some("hot"));
}

#[test]
fn cli_artifact_type_str_table() {
    let ctx = test_context();
    let table = systemprompt_models::TableArtifact::new(vec![], &ctx);
    let cli = CliArtifact::table(table);
    assert_eq!(cli.artifact_type_str(), "table");
}

#[test]
fn cli_artifact_type_str_text() {
    let ctx = test_context();
    let text = systemprompt_models::artifacts::text::TextArtifact::new("hello", &ctx);
    let cli = CliArtifact::text(text);
    assert_eq!(cli.artifact_type_str(), "text");
}

#[test]
fn cli_artifact_title_for_text() {
    let ctx = test_context();
    let text =
        systemprompt_models::artifacts::text::TextArtifact::new("content", &ctx).with_title("My Title");
    let cli = CliArtifact::text(text);
    assert_eq!(cli.title().as_deref(), Some("My Title"));
}

#[test]
fn cli_artifact_title_for_table_is_none() {
    let ctx = test_context();
    let table = systemprompt_models::TableArtifact::new(vec![], &ctx);
    let cli = CliArtifact::table(table);
    assert!(cli.title().is_none());
}

#[test]
fn artifact_type_display() {
    assert_eq!(ArtifactType::Text.to_string(), "text");
    assert_eq!(ArtifactType::Table.to_string(), "table");
    assert_eq!(ArtifactType::Chart.to_string(), "chart");
    assert_eq!(ArtifactType::Dashboard.to_string(), "dashboard");
    assert_eq!(ArtifactType::List.to_string(), "list");
    assert_eq!(
        ArtifactType::Custom("custom".to_string()).to_string(),
        "custom"
    );
}

#[test]
fn artifact_type_equality() {
    assert_eq!(ArtifactType::Text, ArtifactType::Text);
    assert_ne!(ArtifactType::Text, ArtifactType::Table);
}

#[test]
fn chart_type_default() {
    assert!(matches!(ChartType::default(), ChartType::Line));
}

#[test]
fn axis_type_default() {
    assert!(matches!(AxisType::default(), AxisType::Linear));
}

#[test]
fn column_type_all_variants_serialize() {
    let variants = vec![
        (ColumnType::String, "string"),
        (ColumnType::Integer, "integer"),
        (ColumnType::Number, "number"),
        (ColumnType::Currency, "currency"),
        (ColumnType::Percentage, "percentage"),
        (ColumnType::Date, "date"),
        (ColumnType::Boolean, "boolean"),
        (ColumnType::Link, "link"),
    ];
    for (variant, expected) in variants {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{}\"", expected));
    }
}

#[test]
fn chart_section_data_new() {
    let data = systemprompt_models::artifacts::dashboard::ChartSectionData::new(
        "bar",
        vec!["Jan".to_string(), "Feb".to_string()],
        vec![ChartDataset::new("Sales", vec![10.0, 20.0])],
    );
    assert_eq!(data.chart_type, "bar");
    assert_eq!(data.labels.len(), 2);
    assert_eq!(data.datasets.len(), 1);
}

#[test]
fn layout_mode_serde_roundtrip() {
    let modes = vec![LayoutMode::Vertical, LayoutMode::Grid, LayoutMode::Tabs];
    for mode in modes {
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: LayoutMode = serde_json::from_str(&json).unwrap();
        assert_eq!(
            std::mem::discriminant(&mode),
            std::mem::discriminant(&deserialized)
        );
    }
}

#[test]
fn section_type_serde_roundtrip() {
    let types = vec![
        SectionType::MetricsCards,
        SectionType::Table,
        SectionType::Chart,
        SectionType::Timeline,
        SectionType::Status,
        SectionType::List,
    ];
    for st in types {
        let json = serde_json::to_string(&st).unwrap();
        let deserialized: SectionType = serde_json::from_str(&json).unwrap();
        assert_eq!(
            std::mem::discriminant(&st),
            std::mem::discriminant(&deserialized)
        );
    }
}
