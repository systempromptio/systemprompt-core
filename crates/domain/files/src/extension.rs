//! Extension registration for the files domain.
//!
//! Registers the `files`, `content_files` and document store schemas, and the
//! `report_document_revisions` and `report_document_sharing` views, with the
//! platform extension framework, with the user-purge rows of the document
//! store and issue report tables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

use crate::document_schemas::DOCUMENT_SCHEMAS;
use crate::documents::repository::DOCUMENT_USER_PURGES;
use crate::issues::repository::ISSUE_USER_PURGES;

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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(
            SchemaDefinition::new("files", include_str!("../schema/files.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "path".into(),
                    "mime_type".into(),
                    "created_at".into(),
                ]),
        )
        .schema(
            SchemaDefinition::new("content_files", include_str!("../schema/content_files.sql"))
                .with_required_columns(vec!["id".into(), "content_id".into(), "file_id".into()]),
        );
        for (table, sql, columns) in DOCUMENT_SCHEMAS {
            c.schema(
                SchemaDefinition::new(*table, *sql)
                    .with_required_columns(columns.iter().map(|c| (*c).into()).collect()),
            );
        }
        for view in [
            include_str!("../schema/report_document_revisions.sql"),
            include_str!("../schema/report_document_sharing.sql"),
        ] {
            c.schema(SchemaDefinition::sql_only(view));
        }
        c.migrations(extension_migrations!());
        for purge in DOCUMENT_USER_PURGES.iter().chain(ISSUE_USER_PURGES) {
            c.user_purge(*purge);
        }
        Ok(())
    }
}
