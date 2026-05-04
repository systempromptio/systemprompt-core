//! [`ToolContent`] — one content fragment in a tool result.

use serde::{Deserialize, Serialize};

/// One content fragment in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Plain text fragment.
    Text {
        /// The text payload.
        text: String,
    },
    /// Inline base64-encoded image.
    Image {
        /// Base64-encoded image bytes.
        data: String,
        /// MIME type of `data`.
        mime_type: String,
    },
    /// Reference to an external resource.
    Resource {
        /// Resource URI.
        uri: String,
        /// MIME type of the resource, when known.
        mime_type: Option<String>,
    },
}

impl ToolContent {
    /// Build a [`ToolContent::Text`] fragment.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}
