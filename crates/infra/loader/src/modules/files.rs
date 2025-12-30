use std::path::PathBuf;
use systemprompt_models::modules::{Module, ModuleSchema, SchemaSource};

pub fn define() -> Module {
    Module {
        uuid: uuid(),
        name: "files".into(),
        version: "0.0.1".into(),
        display_name: "File Management".into(),
        description: Some(
            "Centralized file management for SystemPrompt with content associations".into(),
        ),
        weight: Some(25),
        dependencies: vec!["database".into(), "log".into(), "content".into()],
        schemas: Some(vec![
            ModuleSchema {
                table: "files".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/files/schema/files.sql").into(),
                ),
                required_columns: vec![
                    "id".into(),
                    "file_path".into(),
                    "public_url".into(),
                    "mime_type".into(),
                ],
            },
            ModuleSchema {
                table: "content_files".into(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/files/schema/content_files.sql").into(),
                ),
                required_columns: vec!["id".into(), "content_id".into(), "file_id".into()],
            },
            ModuleSchema {
                table: String::new(),
                sql: SchemaSource::Inline(
                    include_str!("../../../../domain/files/schema/ai_image_analytics.sql").into(),
                ),
                required_columns: vec![],
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
    "files-module-0001-0001-000000000001".into()
}
