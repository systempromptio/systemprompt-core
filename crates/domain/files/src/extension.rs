//! Extension registration for the files domain.
//!
//! Registers the `files`, `content_files` and document store schemas, and the
//! `report_document_revisions` view, with the platform extension framework,
//! and declares dependencies on the `users` and `content` extensions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

use crate::document_schemas::DOCUMENT_SCHEMAS;

const WEB_ASSETS: &[(&str, &str, AssetType)] = &[
    (
        "js/documents.js",
        "assets/documents/js/documents.js",
        AssetType::JavaScript,
    ),
    (
        "js/documents-api.js",
        "assets/documents/js/documents-api.js",
        AssetType::JavaScript,
    ),
    (
        "js/documents-events.js",
        "assets/documents/js/documents-events.js",
        AssetType::JavaScript,
    ),
    (
        "js/documents-html.js",
        "assets/documents/js/documents-html.js",
        AssetType::JavaScript,
    ),
    (
        "js/documents-patch.js",
        "assets/documents/js/documents-patch.js",
        AssetType::JavaScript,
    ),
    (
        "js/documents-element.js",
        "assets/documents/js/documents-element.js",
        AssetType::JavaScript,
    ),
    (
        "js/documents-collab.js",
        "assets/documents/js/documents-collab.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-document-browser.js",
        "assets/documents/js/sp-document-browser.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-document-preview.js",
        "assets/documents/js/sp-document-preview.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-presence-bar.js",
        "assets/documents/js/sp-presence-bar.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-document-editor.js",
        "assets/documents/js/sp-document-editor.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-sheet-grid.js",
        "assets/documents/js/sp-sheet-grid.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-slide-text.js",
        "assets/documents/js/sp-slide-text.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-share-dialog.js",
        "assets/documents/js/sp-share-dialog.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-proposal-list.js",
        "assets/documents/js/sp-proposal-list.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-comments.js",
        "assets/documents/js/sp-comments.js",
        AssetType::JavaScript,
    ),
    (
        "js/sp-history.js",
        "assets/documents/js/sp-history.js",
        AssetType::JavaScript,
    ),
    (
        "css/documents.css",
        "assets/documents/css/documents.css",
        AssetType::Css,
    ),
    (
        "css/documents-panels.css",
        "assets/documents/css/documents-panels.css",
        AssetType::Css,
    ),
    (
        "wasm/loro.wasm",
        "assets/documents/wasm/loro.wasm",
        AssetType::Wasm,
    ),
    (
        "wasm/loro.js",
        "assets/documents/wasm/loro.js",
        AssetType::JavaScript,
    ),
    (
        "wasm/loro_wasm.js",
        "assets/documents/wasm/loro_wasm.js",
        AssetType::JavaScript,
    ),
    (
        "wasm/loro-snippet.js",
        "assets/documents/wasm/loro-snippet.js",
        AssetType::JavaScript,
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
        .chain(std::iter::once(SchemaDefinition::sql_only(include_str!(
            "../schema/report_document_revisions.sql"
        ))))
        .collect()
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "content"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn declares_assets(&self) -> bool {
        true
    }

    fn required_assets(&self, _paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        WEB_ASSETS
            .iter()
            .map(|(source, destination, kind)| {
                AssetDefinition::builder(root.join(source), destination, *kind)
                    .optional()
                    .build()
            })
            .collect()
    }
}

register_extension!(FilesExtension);
