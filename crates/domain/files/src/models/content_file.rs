use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::ContentId;

use crate::error::{FilesError, FilesResult};

/// Role a file plays in relation to a piece of content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    /// Featured image used as the primary visual.
    Featured,
    /// Generic attachment associated with the content.
    #[default]
    Attachment,
    /// Inline-embedded asset referenced from the content body.
    Inline,
    /// Open Graph image surfaced to social previews.
    OgImage,
    /// Smaller thumbnail variant used in listings.
    Thumbnail,
}

impl FileRole {
    /// String representation persisted in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Featured => "featured",
            Self::Attachment => "attachment",
            Self::Inline => "inline",
            Self::OgImage => "og_image",
            Self::Thumbnail => "thumbnail",
        }
    }

    /// Parses a role from its string representation; case-insensitive.
    pub fn parse(s: &str) -> FilesResult<Self> {
        match s.to_lowercase().as_str() {
            "featured" => Ok(Self::Featured),
            "attachment" => Ok(Self::Attachment),
            "inline" => Ok(Self::Inline),
            "og_image" => Ok(Self::OgImage),
            "thumbnail" => Ok(Self::Thumbnail),
            other => Err(FilesError::Validation(format!(
                "invalid file role: {other}"
            ))),
        }
    }
}

impl std::fmt::Display for FileRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Database row associating a [`super::File`] with a piece of content via
/// [`FileRole`].
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContentFile {
    /// Primary key.
    pub id: i32,
    /// Owning content row.
    pub content_id: ContentId,
    /// Referenced file row (UUID).
    pub file_id: uuid::Uuid,
    /// Stringified [`FileRole`]; parse via [`ContentFile::parsed_role`].
    pub role: String,
    /// Display order among files in the same role on the same content.
    pub display_order: i32,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl ContentFile {
    /// Returns the strongly-typed [`FileRole`] for this association.
    pub fn parsed_role(&self) -> FilesResult<FileRole> {
        FileRole::parse(&self.role)
    }
}
