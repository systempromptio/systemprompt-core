//! Compute the diff between content stored on disk (markdown + frontmatter)
//! and in the database for one content source.

use super::compute_content_hash;
use crate::error::{SyncError, SyncResult};
use crate::models::{ContentDiffItem, ContentDiffResult, DiffStatus, DiskContent};
use std::collections::HashMap;
use std::path::Path;
use systemprompt_content::models::Content;
use systemprompt_content::repository::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{LocaleCode, SourceId};
use tracing::warn;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct ContentDiffCalculator {
    content_repo: ContentRepository,
}

impl ContentDiffCalculator {
    pub fn new(db: &DbPool) -> SyncResult<Self> {
        Ok(Self {
            content_repo: ContentRepository::new(db).map_err(SyncError::internal)?,
        })
    }

    pub async fn calculate_diff(
        &self,
        source_id: &SourceId,
        disk_path: &Path,
        allowed_types: &[String],
    ) -> SyncResult<ContentDiffResult> {
        let db_content = self
            .content_repo
            .list_by_source(source_id, &LocaleCode::new("en"))
            .await
            .map_err(SyncError::internal)?;
        let db_map: HashMap<String, Content> = db_content
            .into_iter()
            .map(|c| (c.slug.clone(), c))
            .collect();

        let disk_items = Self::scan_disk_content(disk_path, allowed_types);

        let mut result = ContentDiffResult {
            source_id: source_id.clone(),
            ..Default::default()
        };

        for (slug, disk_item) in &disk_items {
            let disk_hash = compute_content_hash(&disk_item.body, &disk_item.title);

            match db_map.get(slug) {
                None => {
                    result.added.push(ContentDiffItem {
                        slug: slug.clone(),
                        source_id: source_id.clone(),
                        status: DiffStatus::Added,
                        disk_hash: Some(disk_hash),
                        db_hash: None,
                        disk_updated_at: None,
                        db_updated_at: None,
                        title: Some(disk_item.title.clone()),
                    });
                },
                Some(db_item) => {
                    if db_item.version_hash == disk_hash {
                        result.unchanged += 1;
                    } else {
                        result.modified.push(ContentDiffItem {
                            slug: slug.clone(),
                            source_id: source_id.clone(),
                            status: DiffStatus::Modified,
                            disk_hash: Some(disk_hash),
                            db_hash: Some(db_item.version_hash.clone()),
                            disk_updated_at: None,
                            db_updated_at: Some(db_item.updated_at),
                            title: Some(disk_item.title.clone()),
                        });
                    }
                },
            }
        }

        for (slug, db_item) in &db_map {
            if !disk_items.contains_key(slug) {
                result.removed.push(ContentDiffItem {
                    slug: slug.clone(),
                    source_id: source_id.clone(),
                    status: DiffStatus::Removed,
                    disk_hash: None,
                    db_hash: Some(db_item.version_hash.clone()),
                    disk_updated_at: None,
                    db_updated_at: Some(db_item.updated_at),
                    title: Some(db_item.title.clone()),
                });
            }
        }

        Ok(result)
    }

    fn scan_disk_content(path: &Path, allowed_types: &[String]) -> HashMap<String, DiskContent> {
        let mut items = HashMap::new();

        if !path.exists() {
            return items;
        }

        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    tracing::warn!(error = %err, "Failed to read directory entry during sync");
                    None
                },
            })
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        {
            let file_path = entry.path();
            match parse_content_file(file_path, allowed_types) {
                Ok(Some(content)) => {
                    items.insert(content.slug.clone(), content);
                },
                Ok(None) => {},
                Err(e) => {
                    warn!("Failed to parse {}: {}", file_path.display(), e);
                },
            }
        }

        items
    }
}

fn parse_content_file(path: &Path, allowed_types: &[String]) -> SyncResult<Option<DiskContent>> {
    let content = std::fs::read_to_string(path)?;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(SyncError::invalid_input("Invalid frontmatter format"));
    }

    let frontmatter: serde_yaml::Value = serde_yaml::from_str(parts[1])?;
    let body = parts[2].trim().to_owned();

    let kind = frontmatter
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SyncError::invalid_input("Missing kind in frontmatter"))?;

    if !allowed_types.iter().any(|t| t == kind) {
        return Ok(None);
    }

    let slug = frontmatter
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SyncError::invalid_input("Missing slug in frontmatter"))?.to_owned();

    let title = frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SyncError::invalid_input("Missing title in frontmatter"))?.to_owned();

    Ok(Some(DiskContent { slug, title, body }))
}
