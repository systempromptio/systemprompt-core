//! The single row → [`Part`] converter shared by every message-part and
//! artifact-part read path, so a malformed row is rejected identically
//! whether a task is fetched singly, in a batch, or by context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::a2a::{Part, PartContent};
use crate::models::{ArtifactPartRow, MessagePart};
use systemprompt_traits::RepositoryError;

pub(crate) const PART_KIND_TEXT: &str = "text";
pub(crate) const PART_KIND_FILE: &str = "file";
pub(crate) const PART_KIND_DATA: &str = "data";

pub(crate) trait PartColumns {
    fn part_kind(&self) -> &str;
    fn text_content(&self) -> Option<&str>;
    fn file_name(&self) -> Option<&str>;
    fn file_mime_type(&self) -> Option<&str>;
    fn file_uri(&self) -> Option<&str>;
    fn file_bytes(&self) -> Option<&str>;
    // JSON: JSONB `data_content` column — an A2A data part holds any JSON value.
    fn data_content(&self) -> Option<&serde_json::Value>;
    // JSON: JSONB `metadata` column — open A2A metadata object.
    fn metadata(&self) -> Option<&serde_json::Value>;
}

macro_rules! impl_part_columns {
    ($row:ty) => {
        impl PartColumns for $row {
            fn part_kind(&self) -> &str {
                &self.part_kind
            }
            fn text_content(&self) -> Option<&str> {
                self.text_content.as_deref()
            }
            fn file_name(&self) -> Option<&str> {
                self.file_name.as_deref()
            }
            fn file_mime_type(&self) -> Option<&str> {
                self.file_mime_type.as_deref()
            }
            fn file_uri(&self) -> Option<&str> {
                self.file_uri.as_deref()
            }
            fn file_bytes(&self) -> Option<&str> {
                self.file_bytes.as_deref()
            }
            // JSON: JSONB `data_content` column — an A2A data part holds any JSON value.
            fn data_content(&self) -> Option<&serde_json::Value> {
                self.data_content.as_ref()
            }
            // JSON: JSONB `metadata` column — open A2A metadata object.
            fn metadata(&self) -> Option<&serde_json::Value> {
                self.metadata.as_ref()
            }
        }
    };
}

impl_part_columns!(MessagePart);
impl_part_columns!(ArtifactPartRow);

pub(crate) fn part_from_row(row: &impl PartColumns) -> Result<Part, RepositoryError> {
    let content = match row.part_kind() {
        PART_KIND_TEXT => {
            let text = row.text_content().ok_or_else(|| {
                RepositoryError::invalid_data("text_content", "missing for a text part")
            })?;
            PartContent::Text(text.to_owned())
        },
        PART_KIND_FILE => match (row.file_bytes(), row.file_uri()) {
            (Some(bytes), _) => PartContent::Raw(bytes.to_owned()),
            (None, Some(uri)) => PartContent::Url(uri.to_owned()),
            (None, None) => {
                return Err(RepositoryError::invalid_data(
                    "file_uri",
                    "file part has neither file_uri nor file_bytes",
                ));
            },
        },
        PART_KIND_DATA => {
            let data = row.data_content().ok_or_else(|| {
                RepositoryError::invalid_data("data_content", "missing for a data part")
            })?;
            PartContent::Data(data.clone())
        },
        other => {
            return Err(RepositoryError::invalid_data(
                "part_kind",
                format!("unknown part kind {other}"),
            ));
        },
    };
    let metadata = metadata_object(row.metadata().cloned());
    Ok(Part {
        content,
        filename: row.file_name().map(str::to_owned),
        media_type: row.file_mime_type().map(str::to_owned),
        metadata,
    })
}

// JSON: JSONB `metadata` columns hold an open A2A metadata object; an empty
// object is the column default and reads as absent.
pub(crate) fn metadata_object(
    value: Option<serde_json::Value>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match value {
        Some(serde_json::Value::Object(map)) if !map.is_empty() => Some(map),
        _ => None,
    }
}

#[derive(Debug)]
pub(crate) struct PartRowValues<'a> {
    pub(crate) part_kind: &'static str,
    pub(crate) text_content: Option<&'a str>,
    pub(crate) file_name: Option<&'a str>,
    pub(crate) file_mime_type: Option<&'a str>,
    pub(crate) file_uri: Option<&'a str>,
    pub(crate) file_bytes: Option<&'a str>,
    // JSON: an A2A data part's payload is arbitrary JSON by protocol.
    pub(crate) data_content: Option<serde_json::Value>,
    // JSON: A2A part metadata is an open JSON object by protocol.
    pub(crate) metadata: Option<serde_json::Value>,
}

pub(crate) fn row_values(part: &Part) -> PartRowValues<'_> {
    let (part_kind, text_content, file_uri, file_bytes, data_content) = match &part.content {
        PartContent::Text(text) => (PART_KIND_TEXT, Some(text.as_str()), None, None, None),
        PartContent::Raw(bytes) => (PART_KIND_FILE, None, None, Some(bytes.as_str()), None),
        PartContent::Url(url) => (PART_KIND_FILE, None, Some(url.as_str()), None, None),
        PartContent::Data(data) => (PART_KIND_DATA, None, None, None, Some(data.clone())),
    };
    PartRowValues {
        part_kind,
        text_content,
        file_name: part.filename.as_deref(),
        file_mime_type: part.media_type.as_deref(),
        file_uri,
        file_bytes,
        data_content,
        metadata: part.metadata.clone().map(serde_json::Value::Object),
    }
}

pub(crate) fn parts_from_rows<R: PartColumns>(rows: &[R]) -> Result<Vec<Part>, RepositoryError> {
    rows.iter().map(part_from_row).collect()
}
