//! Constructs [`Content`] records from parsed frontmatter and computes the
//! version hash used to detect content changes between ingestion passes.

use crate::error::ContentError;
use crate::models::{Content, ContentLinkMetadata, ContentMetadata};
use sha2::{Digest, Sha256};
use systemprompt_identifiers::{CategoryId, ContentId, LocaleCode, SourceId};

pub fn create_content_from_metadata(
    metadata: &ContentMetadata,
    content_text: &str,
    source_id: SourceId,
    category_id: String,
) -> Result<Content, ContentError> {
    let id = ContentId::new(uuid::Uuid::new_v4().to_string());
    let slug = metadata.slug.clone();

    let published_at = chrono::NaiveDate::parse_from_str(&metadata.published_at, "%Y-%m-%d")
        .map_err(|e| {
            ContentError::Parse(format!(
                "Invalid published_at date '{}': {}",
                metadata.published_at, e
            ))
        })?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ContentError::Parse("Failed to create datetime".to_string()))?
        .and_local_timezone(chrono::Utc)
        .single()
        .ok_or_else(|| ContentError::Parse("Ambiguous timezone conversion".to_string()))?;

    let links_vec: Vec<ContentLinkMetadata> = metadata
        .links
        .iter()
        .map(|link| ContentLinkMetadata {
            title: link.title.clone(),
            url: link.url.clone(),
        })
        .collect();

    let links = serde_json::to_value(&links_vec)?;

    Ok(Content {
        id,
        slug,
        locale: metadata
            .locale
            .clone()
            .unwrap_or_else(|| LocaleCode::new("en")),
        title: metadata.title.clone(),
        description: metadata.description.clone(),
        body: content_text.to_string(),
        author: metadata.author.clone(),
        published_at,
        keywords: metadata.keywords.clone(),
        kind: metadata.kind.clone(),
        image: metadata.image.clone(),
        category_id: Some(CategoryId::new(category_id)),
        source_id,
        version_hash: String::new(),
        public: metadata.public.unwrap_or(true),
        links,
        updated_at: chrono::Utc::now(),
    })
}

pub fn compute_version_hash(content: &Content) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.title.as_bytes());
    hasher.update(content.body.as_bytes());
    hasher.update(content.description.as_bytes());
    hasher.update(content.author.as_bytes());
    hasher.update(content.published_at.to_string().as_bytes());
    hasher.update(content.public.to_string().as_bytes());
    hex::encode(hasher.finalize())
}
