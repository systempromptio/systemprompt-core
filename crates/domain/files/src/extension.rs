//! Extension registration for the files domain.
//!
//! Registers the `files`, `content_files` and document store schemas, and the
//! `report_document_revisions` and `report_document_sharing` views, with the
//! platform extension framework, and declares dependencies on the `users`
//! and `content` extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

use crate::document_schemas::DOCUMENT_SCHEMAS;

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

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new("files", include_str!("../schema/files.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "path".into(),
                    "mime_type".into(),
                    "created_at".into(),
                ]),
            SchemaDefinition::new("content_files", include_str!("../schema/content_files.sql"))
                .with_required_columns(vec!["id".into(), "content_id".into(), "file_id".into()]),
        ]
        .into_iter()
        .chain(DOCUMENT_SCHEMAS.iter().map(|(table, sql, columns)| {
            SchemaDefinition::new(*table, *sql)
                .with_required_columns(columns.iter().map(|c| (*c).into()).collect())
        }))
        .chain(
            [
                include_str!("../schema/report_document_revisions.sql"),
                include_str!("../schema/report_document_sharing.sql"),
            ]
            .map(SchemaDefinition::sql_only),
        )
        .collect()
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "content"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(FilesExtension);
