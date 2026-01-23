use systemprompt_extension::prelude::*;

#[derive(Debug, Clone, Copy, Default)]
pub struct FilesExtension;

impl Extension for FilesExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "files",
            name: "Files",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn migration_weight(&self) -> u32 {
        50
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::inline("files", include_str!("../schema/files.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "filename".into(),
                    "mime_type".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::inline("content_files", include_str!("../schema/content_files.sql"))
                .with_required_columns(vec!["id".into(), "content_id".into(), "file_id".into()]),
            SchemaDefinition::inline(
                "ai_image_analytics",
                include_str!("../schema/ai_image_analytics.sql"),
            ),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "content"]
    }
}

register_extension!(FilesExtension);
