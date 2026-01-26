use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId, TagId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    #[default]
    Article,
    Guide,
    Tutorial,
}

impl ContentKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Guide => "guide",
            Self::Tutorial => "tutorial",
        }
    }
}

impl std::fmt::Display for ContentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Content {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub author: String,
    pub published_at: DateTime<Utc>,
    pub keywords: String,
    pub kind: String,
    pub image: Option<String>,
    pub category_id: Option<CategoryId>,
    pub source_id: SourceId,
    pub version_hash: String,
    pub public: bool,
    #[serde(default)]
    pub links: JsonValue,
    pub updated_at: DateTime<Utc>,
}

impl Content {
    pub fn links_metadata(&self) -> Result<Vec<ContentLinkMetadata>, serde_json::Error> {
        serde_json::from_value(self.links.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummary {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMetadata {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    pub published_at: String,
    pub slug: String,
    #[serde(default)]
    pub keywords: String,
    pub kind: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub links: Vec<ContentLinkMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentLinkMetadata {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub slug: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionReport {
    pub files_found: usize,
    pub files_processed: usize,
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub would_create: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub would_update: Vec<String>,
    #[serde(default)]
    pub unchanged_count: usize,
}

impl IngestionReport {
    pub const fn new() -> Self {
        Self {
            files_found: 0,
            files_processed: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
            would_create: Vec::new(),
            would_update: Vec::new(),
            unchanged_count: 0,
        }
    }

    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Default for IngestionReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IngestionOptions {
    pub override_existing: bool,
    pub recursive: bool,
    pub dry_run: bool,
}

impl IngestionOptions {
    pub const fn with_override(mut self, override_existing: bool) -> Self {
        self.override_existing = override_existing;
        self
    }

    pub const fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

#[derive(Debug, Clone)]
pub struct IngestionSource<'a> {
    pub source_id: &'a SourceId,
    pub category_id: &'a CategoryId,
}

impl<'a> IngestionSource<'a> {
    pub const fn new(source_id: &'a SourceId, category_id: &'a CategoryId) -> Self {
        Self {
            source_id,
            category_id,
        }
    }
}
