//! Extension registration for the files domain.
//!
//! Registers the `files`, `content_files` and document store schemas with the
//! platform extension framework and declares dependencies on the `users` and
//! `content` extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

type SchemaEntry = (&'static str, &'static str, &'static [&'static str]);

const DOCUMENT_SCHEMAS: &[SchemaEntry] = &[
    (
        "document_folders",
        include_str!("../schema/document_folders.sql"),
        &["id", "parent_id", "name", "ancestor_ids", "owner_user_id"],
    ),
    (
        "document_blobs",
        include_str!("../schema/document_blobs.sql"),
        &["content_sha256", "size_bytes", "storage_id"],
    ),
    (
        "documents",
        include_str!("../schema/documents.sql"),
        &[
            "id",
            "folder_id",
            "name",
            "format",
            "head_revision_id",
            "owner_user_id",
        ],
    ),
    (
        "document_revisions",
        include_str!("../schema/document_revisions.sql"),
        &[
            "id",
            "document_id",
            "content_sha256",
            "author_user_id",
            "created_at",
        ],
    ),
    (
        "document_grants",
        include_str!("../schema/document_grants.sql"),
        &[
            "id",
            "document_id",
            "folder_id",
            "subject_kind",
            "subject_value",
            "access_level",
        ],
    ),
    (
        "document_locks",
        include_str!("../schema/document_locks.sql"),
        &["document_id", "holder_id", "expires_at"],
    ),
    (
        "document_presence",
        include_str!("../schema/document_presence.sql"),
        &["document_id", "user_id", "source", "expires_at"],
    ),
    (
        "document_edit_sessions",
        include_str!("../schema/document_edit_sessions.sql"),
        &["id", "document_id", "closed_at"],
    ),
    (
        "document_collab_updates",
        include_str!("../schema/document_collab_updates.sql"),
        &["session_id", "seq", "payload"],
    ),
    (
        "document_text_index",
        include_str!("../schema/document_text_index.sql"),
        &["document_id", "revision_id", "body"],
    ),
];

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
