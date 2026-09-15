//! The single row → [`Part`] converter shared by every message-part and
//! artifact-part read path, so a malformed row is rejected identically
//! whether a task is fetched singly, in a batch, or by context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::a2a::{DataPart, FileContent, FilePart, Part, TextPart};
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
    fn data_content(&self) -> Option<&serde_json::Value>;
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
            fn data_content(&self) -> Option<&serde_json::Value> {
                self.data_content.as_ref()
            }
        }
    };
}

impl_part_columns!(MessagePart);
impl_part_columns!(ArtifactPartRow);

pub(crate) fn part_from_row(row: &impl PartColumns) -> Result<Part, RepositoryError> {
    match row.part_kind() {
        PART_KIND_TEXT => {
            let text = row
                .text_content()
                .ok_or_else(|| RepositoryError::InvalidData("Missing text_content".into()))?;
            Ok(Part::Text(TextPart {
                text: text.to_owned(),
            }))
        },
        PART_KIND_FILE => {
            if row.file_uri().is_none() && row.file_bytes().is_none() {
                return Err(RepositoryError::InvalidData(
                    "File part has neither file_uri nor file_bytes".into(),
                ));
            }
            Ok(Part::File(FilePart {
                file: FileContent {
                    name: row.file_name().map(str::to_owned),
                    mime_type: row.file_mime_type().map(str::to_owned),
                    bytes: row.file_bytes().map(str::to_owned),
                    url: row.file_uri().map(str::to_owned),
                },
            }))
        },
        PART_KIND_DATA => {
            let data_value = row
                .data_content()
                .ok_or_else(|| RepositoryError::InvalidData("Missing data_content".into()))?;
            let serde_json::Value::Object(data) = data_value else {
                return Err(RepositoryError::InvalidData(
                    "Data content must be a JSON object".into(),
                ));
            };
            Ok(Part::Data(DataPart { data: data.clone() }))
        },
        other => Err(RepositoryError::InvalidData(format!(
            "Unknown part kind: {other}"
        ))),
    }
}

pub(crate) fn parts_from_rows<R: PartColumns>(rows: &[R]) -> Result<Vec<Part>, RepositoryError> {
    rows.iter().map(part_from_row).collect()
}
