//! The kinds of document a user or the agent creates from the instance's
//! blank templates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

/// A kind of new document, named by the template the instance creates it from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentKind {
    #[default]
    Markdown,
    Text,
    Html,
    Word,
    Spreadsheet,
    Presentation,
}

impl DocumentKind {
    pub const ALL: [Self; 6] = [
        Self::Markdown,
        Self::Word,
        Self::Spreadsheet,
        Self::Presentation,
        Self::Html,
        Self::Text,
    ];

    pub const fn template_id(self) -> &'static str {
        match self {
            Self::Markdown => "blank-markdown",
            Self::Text => "blank-text",
            Self::Html => "blank-html",
            Self::Word => "blank-docx",
            Self::Spreadsheet => "blank-xlsx",
            Self::Presentation => "blank-pptx",
        }
    }
}
