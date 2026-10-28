//! The versioned, serialisable view of a document's model: what
//! `documents_read` returns with `view: model`, what `documents_create`
//! accepts as `content.model`, and what `/api/v1/documents/schema` describes.
//!
//! `version` is [`RICH_TEXT_VERSION`]; a reader refuses a newer version it
//! does not know rather than dropping what it cannot hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::rich_text::{RICH_TEXT_VERSION, RichText};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "model view {kind} v{version} is not supported (this server reads rich_text up to v{supported})"
)]
pub struct UnsupportedModelView {
    pub kind: String,
    pub version: u32,
    pub supported: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelViewV2 {
    #[serde(default = "current")]
    pub version: u32,
    #[serde(default = "rich_text_kind")]
    pub kind: String,
    #[serde(flatten)]
    pub model: RichText,
}

impl ModelViewV2 {
    #[must_use]
    pub fn new(model: RichText) -> Self {
        Self {
            version: RICH_TEXT_VERSION,
            kind: rich_text_kind(),
            model,
        }
    }

    pub fn into_model(self) -> Result<RichText, UnsupportedModelView> {
        if self.version > RICH_TEXT_VERSION || self.kind != "rich_text" {
            return Err(UnsupportedModelView {
                kind: self.kind,
                version: self.version,
                supported: RICH_TEXT_VERSION,
            });
        }
        Ok(self.model)
    }
}

const fn current() -> u32 {
    RICH_TEXT_VERSION
}

fn rich_text_kind() -> String {
    "rich_text".to_owned()
}

#[must_use]
pub fn model_view_schema() -> schemars::Schema {
    schemars::schema_for!(ModelViewV2)
}
