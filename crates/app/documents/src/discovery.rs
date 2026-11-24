//! The `endpoints.documents` block of the root discovery document: whether
//! the document store is on and, when it is, where its routes are and the
//! byte limits an upload, a part of one and an inline upload stay within, and
//! the rendered-HTML policy (`renderedHtml`: `scripts`, `remoteOrigins` and
//! the `agentAuthored` tier), and whether a document may be shared outside
//! the instance (`publicLinks`, `externalInvites`), so a client learns the
//! Drive from discovery instead of probing for a `404`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_files::{DocumentsConfig, RenderedHtmlConfig};
use systemprompt_models::modules::ApiPaths;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentsDiscovery {
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
    rendered_html: RenderedHtmlConfig,
    public_links: bool,
    external_invites: bool,
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
    #[must_use]
    pub fn of_state(state: &systemprompt_provider_contracts::Dependencies, base: &str) -> Self {
        state
            .find::<std::sync::Arc<systemprompt_files::FilesConfig>>()
            .map_or(
                Self {
                    enabled: false,
                    surface: None,
                },
                |config| Self::new(config.documents_config(), base),
            )
    }

    pub fn new(config: &DocumentsConfig, base: &str) -> Self {
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
                rendered_html: config.rendered_html.clone(),
                public_links: config.external_sharing.public_links,
                external_invites: config.external_sharing.invites,
            }),
        }
    }
}
