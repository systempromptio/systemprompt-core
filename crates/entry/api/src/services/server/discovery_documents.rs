//! The `endpoints.documents` block of the root discovery document: whether
//! the document store is on and, when it is, where its routes are and the
//! byte limits an upload, a part of one and an inline upload stay within, so
//! a client learns the Drive from discovery instead of probing for a `404`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_files::DocumentsConfig;
use systemprompt_models::modules::ApiPaths;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DocumentsDiscovery {
    enabled: bool,
    #[serde(flatten)]
    surface: Option<DocumentsSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentsSurface {
    href: String,
    folders: String,
    uploads: String,
    limits: DocumentsLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct DocumentsLimits {
    #[serde(rename = "maxUploadBytes")]
    max_upload: u64,
    #[serde(rename = "inlineMaxBytes")]
    inline_max: u64,
    #[serde(rename = "uploadPartBytes")]
    upload_part: u64,
}

impl DocumentsDiscovery {
    pub(super) fn new(config: &DocumentsConfig, base: &str) -> Self {
        if !config.is_enabled() {
            return Self {
                enabled: false,
                surface: None,
            };
        }
        Self {
            enabled: true,
            surface: Some(DocumentsSurface {
                href: format!("{base}{}", ApiPaths::DOCUMENTS),
                folders: format!("{base}{}", ApiPaths::DOCUMENT_FOLDERS),
                uploads: format!("{base}{}", ApiPaths::UPLOADS),
                limits: DocumentsLimits {
                    max_upload: config.max_upload_bytes,
                    inline_max: config.uploads.inline_max_bytes,
                    upload_part: config.uploads.part_size_bytes,
                },
            }),
        }
    }
}
