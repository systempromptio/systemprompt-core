//! Markdown ingestion: scans content directories, parses frontmatter, and
//! reconciles each file against the content repository.

mod builder;
mod processors;
mod scanner;

use crate::error::ContentError;
use crate::models::{
    CreateContentParams, IngestionOptions, IngestionReport, IngestionSource, UpdateContentParams,
};
use crate::repository::ContentRepository;
use std::path::Path;
use std::sync::Arc;
use systemprompt_database::DbPool;

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
    db_pool: DbPool,
}

impl IngestionService {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            content_repo: ContentRepository::new(db)?,
            db_pool: Arc::clone(db),
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
                        IngestFileResult::Skipped => report.skipped_count += 1,
                        IngestFileResult::Created | IngestFileResult::Updated => {},
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
        let parsed = scanner::parse_frontmatter(&markdown_text)?;

        let resolved_category_id = parsed
            .metadata
            .category
            .clone()
            .unwrap_or_else(|| source.category_id.to_string());

        let new_content = builder::create_content_from_metadata(
            &parsed.metadata,
            &parsed.body,
            source.source_id.clone(),
            resolved_category_id,
        )?;

        let existing_content = self
            .content_repo
            .get_by_source_and_slug(
                &new_content.source_id,
                &new_content.slug,
                &new_content.locale,
            )
            .await?;

        let slug = new_content.slug.clone();
        let new_hash = builder::compute_version_hash(&new_content);

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
                .with_locale(new_content.locale.clone())
                .with_author(new_content.author.clone())
                .with_published_at(new_content.published_at)
                .with_keywords(new_content.keywords.clone())
                .with_kind(new_content.kind.clone())
                .with_image(new_content.image.clone())
                .with_category_id(new_content.category_id.clone())
                .with_version_hash(new_hash)
                .with_links(new_content.links.clone())
                .with_public(new_content.public);

                let created_content = self.content_repo.create(&params).await?;

                processors::call_frontmatter_processors(
                    &self.db_pool,
                    created_content.id.as_str(),
                    &slug,
                    source.source_name,
                    &parsed,
                )
                .await;

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
                .with_version_hash(new_hash)
                .with_category_id(Some(new_content.category_id.clone()))
                .with_kind(Some(new_content.kind.clone()))
                .with_author(Some(new_content.author.clone()))
                .with_published_at(Some(new_content.published_at))
                .with_links(Some(new_content.links.clone()))
                .with_public(parsed.metadata.public);
                self.content_repo.update(&update_params).await?;

                processors::call_frontmatter_processors(
                    &self.db_pool,
                    existing.id.as_str(),
                    &slug,
                    source.source_name,
                    &parsed,
                )
                .await;

                Ok(IngestFileResult::Updated)
            },
        }
    }
}
