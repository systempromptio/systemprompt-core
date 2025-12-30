use std::path::PathBuf;
use systemprompt_models::modules::{Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "analytics".into(),
        version: "0.0.1".into(),
        display_name: "Analytics".into(),
        description: Some(
            "Session tracking, behavioral analysis, engagement metrics, and ML feature preparation"
                .into(),
        ),
        weight: Some(100),
        dependencies: vec![],
        schemas: Some(vec![
            ModuleSchema {
                table: "engagement_events".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/analytics/schema/engagement_events.sql")
                        .into(),
                ),
                required_columns: vec!["id".into(), "event_type".into(), "created_at".into()],
            },
            ModuleSchema {
                table: "anomaly_thresholds".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/analytics/schema/anomaly_thresholds.sql")
                        .into(),
                ),
                required_columns: vec!["id".into(), "metric_name".into()],
            },
            ModuleSchema {
                table: "fingerprint_reputation".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/analytics/schema/fingerprint_reputation.sql")
                        .into(),
                ),
                required_columns: vec!["fingerprint".into()],
            },
            ModuleSchema {
                table: "ml_behavioral_features".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/analytics/schema/ml_behavioral_features.sql")
                        .into(),
                ),
                required_columns: vec!["id".into(), "session_id".into()],
            },
        ]),
        seeds: None,
        permissions: None,
        audience: vec![],
        enabled: true,
        api: None,
        path: PathBuf::new(),
    }
}

fn uuid() -> String {
    "analytics-module-0001-0001-000000000001".into()
}
