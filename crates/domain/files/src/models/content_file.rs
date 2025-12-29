use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::ContentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    Featured,
    #[default]
    Attachment,
    Inline,
    OgImage,
    Thumbnail,
}

impl FileRole {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Featured => "featured",
            Self::Attachment => "attachment",
            Self::Inline => "inline",
            Self::OgImage => "og_image",
            Self::Thumbnail => "thumbnail",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "featured" => Ok(Self::Featured),
            "attachment" => Ok(Self::Attachment),
            "inline" => Ok(Self::Inline),
            "og_image" => Ok(Self::OgImage),
            "thumbnail" => Ok(Self::Thumbnail),
            _ => Err(anyhow!("Invalid file role: {s}")),
        }
    }
}


impl std::fmt::Display for FileRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContentFile {
    pub id: i32,
    pub content_id: ContentId,
    pub file_id: uuid::Uuid,
    pub role: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
}

impl ContentFile {
    pub fn parsed_role(&self) -> Result<FileRole> {
        FileRole::parse(&self.role)
    }
}
