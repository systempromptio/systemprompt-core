//! The document formats the store understands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Docx,
    Xlsx,
    Pptx,
    Odt,
    Ods,
    Odp,
    Html,
    Markdown,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown document format `{value}`")]
pub struct UnknownDocumentFormat {
    pub value: String,
}

impl DocumentFormat {
    pub const ALL: [Self; 9] = [
        Self::Docx,
        Self::Xlsx,
        Self::Pptx,
        Self::Odt,
        Self::Ods,
        Self::Odp,
        Self::Html,
        Self::Markdown,
        Self::Text,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Odt => "odt",
            Self::Ods => "ods",
            Self::Odp => "odp",
            Self::Html => "html",
            Self::Markdown => "markdown",
            Self::Text => "text",
        }
    }

    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            },
            Self::Odt => "application/vnd.oasis.opendocument.text",
            Self::Ods => "application/vnd.oasis.opendocument.spreadsheet",
            Self::Odp => "application/vnd.oasis.opendocument.presentation",
            Self::Html => "text/html",
            Self::Markdown => "text/markdown",
            Self::Text => "text/plain",
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Text => "txt",
            other => other.as_str(),
        }
    }

    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        let lower = extension.trim_start_matches('.').to_ascii_lowercase();
        match lower.as_str() {
            "htm" => Some(Self::Html),
            "md" | "markdown" => Some(Self::Markdown),
            "txt" | "text" => Some(Self::Text),
            other => Self::ALL.into_iter().find(|f| f.as_str() == other),
        }
    }

    #[must_use]
    pub const fn is_office(self) -> bool {
        matches!(
            self,
            Self::Docx | Self::Xlsx | Self::Pptx | Self::Odt | Self::Ods | Self::Odp
        )
    }

    #[must_use]
    pub const fn supports_three_way_merge(self) -> bool {
        matches!(self, Self::Html | Self::Markdown | Self::Text)
    }
}

impl fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DocumentFormat {
    type Err = UnknownDocumentFormat;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|f| f.as_str() == s)
            .ok_or_else(|| UnknownDocumentFormat {
                value: s.to_owned(),
            })
    }
}
