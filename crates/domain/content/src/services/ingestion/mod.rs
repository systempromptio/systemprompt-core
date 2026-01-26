mod scanner;

use crate::error::ContentError;
use crate::models::{
    Content, ContentLinkMetadata, ContentMetadata, CreateContentParams, IngestionOptions,
    IngestionReport, IngestionSource, UpdateContentParams,
};
use crate::repository::ContentRepository;
use sha2::{Digest, Sha256};
use std::path::Path;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

#[derive(Debug)]
enum IngestFileResult {
    Created,
    Updated,
    Unchanged,
    Skipped,
    WouldCreate(String),
    WouldUpdate(String),
}

#[derive(Debug)]
pub struct IngestionService {
    content_repo: ContentRepository,
}

impl IngestionService {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            content_repo: ContentRepository::new(db)?,
        })
    }

    pub async fn ingest_directory(
        &self,
        path: &Path,
        source: &IngestionSource<'_>,
        options: IngestionOptions,
    ) -> Result<IngestionReport, ContentError> {
        let mut report = IngestionReport::new();

        let scan_result = scanner::scan_markdown_files(path, options.recursive);
        report.files_found = scan_result.files.len() + scan_result.errors.len();
        report.errors.extend(scan_result.errors);
        report.warnings.extend(scan_result.warnings);

        for file_path in scan_result.files {
            match self
                .ingest_file(
                    &file_path,
                    source,
                    options.override_existing,
                    options.dry_run,
                )
                .await
            {
                Ok(result) => {
                    report.files_processed += 1;
                    match result {
                        IngestFileResult::WouldCreate(slug) => report.would_create.push(slug),
                        IngestFileResult::WouldUpdate(slug) => report.would_update.push(slug),
                        IngestFileResult::Unchanged => report.unchanged_count += 1,
                        IngestFileResult::Created
                        | IngestFileResult::Updated
                        | IngestFileResult::Skipped => {},
                    }
                },
                Err(e) => {
                    report
                        .errors
                        .push(format!("{}: {}", file_path.display(), e));
                },
            }
        }

        Ok(report)
    }

    async fn ingest_file(
        &self,
        path: &Path,
        source: &IngestionSource<'_>,
        override_existing: bool,
        dry_run: bool,
    ) -> Result<IngestFileResult, ContentError> {
        let markdown_text = std::fs::read_to_string(path)?;
        let (metadata, content_text) = scanner::parse_frontmatter(&markdown_text)?;

        let resolved_category_id = metadata
            .category
            .clone()
            .unwrap_or_else(|| source.category_id.to_string());

        let new_content = Self::create_content_from_metadata(
            &metadata,
            &content_text,
            source.source_id.to_string(),
            resolved_category_id,
        )?;

        let existing_content = self
            .content_repo
            .get_by_source_and_slug(&new_content.source_id, &new_content.slug)
            .await?;

        let slug = new_content.slug.clone();
        let new_hash = Self::compute_version_hash(&new_content);

        match existing_content {
            None => {
                if dry_run {
                    return Ok(IngestFileResult::WouldCreate(slug));
                }
                let params = CreateContentParams::new(
                    new_content.slug.clone(),
                    new_content.title.clone(),
                    new_content.description.clone(),
                    new_content.body.clone(),
                    new_content.source_id.clone(),
                )
                .with_author(new_content.author.clone())
                .with_published_at(new_content.published_at)
                .with_keywords(new_content.keywords.clone())
                .with_kind(new_content.kind.clone())
                .with_image(new_content.image.clone())
                .with_category_id(new_content.category_id.clone())
                .with_version_hash(new_hash)
                .with_links(new_content.links.clone());

                self.content_repo.create(&params).await?;
                Ok(IngestFileResult::Created)
            },
            Some(existing) => {
                if existing.version_hash == new_hash {
                    return Ok(IngestFileResult::Unchanged);
                }

                if !override_existing {
                    return Ok(IngestFileResult::Skipped);
                }

                if dry_run {
                    return Ok(IngestFileResult::WouldUpdate(slug));
                }

                let update_params = UpdateContentParams::new(
                    existing.id.clone(),
                    new_content.title.clone(),
                    new_content.description.clone(),
                    new_content.body.clone(),
                )
                .with_keywords(new_content.keywords.clone())
                .with_image(new_content.image.clone())
                .with_version_hash(new_hash);
                self.content_repo.update(&update_params).await?;
                Ok(IngestFileResult::Updated)
            },
        }
    }

    fn create_content_from_metadata(
        metadata: &ContentMetadata,
        content_text: &str,
        source_id: String,
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
            title: metadata.title.clone(),
            description: metadata.description.clone(),
            body: content_text.to_string(),
            author: metadata.author.clone(),
            published_at,
            keywords: metadata.keywords.clone(),
            kind: metadata.kind.clone(),
            image: metadata.image.clone(),
            category_id: Some(CategoryId::new(category_id)),
            source_id: SourceId::new(source_id),
            version_hash: String::new(),
            public: true,
            links,
            updated_at: chrono::Utc::now(),
        })
    }

    fn compute_version_hash(content: &Content) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.title.as_bytes());
        hasher.update(content.body.as_bytes());
        hasher.update(content.description.as_bytes());
        hasher.update(content.author.as_bytes());
        hasher.update(content.published_at.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
