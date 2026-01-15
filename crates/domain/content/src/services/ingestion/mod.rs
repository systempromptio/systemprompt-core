pub mod parser;

use crate::error::ContentError;
use crate::models::{
    Content, ContentKind, ContentLinkMetadata, ContentMetadata, CreateContentParams,
    IngestionOptions, IngestionReport, IngestionSource, UpdateContentParams,
};
use crate::repository::ContentRepository;
use crate::services::validation::validate_content_metadata;
use sha2::{Digest, Sha256};
use std::path::Path;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

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

        let (markdown_files, validation_errors) =
            Self::scan_markdown_files(path, source.allowed_content_types, options.recursive);
        report.files_found = markdown_files.len() + validation_errors.len();
        report.errors.extend(validation_errors);

        for file_path in markdown_files {
            match self
                .ingest_file(
                    &file_path,
                    source,
                    options.override_existing,
                    options.dry_run,
                )
                .await
            {
                Ok(()) => {
                    report.files_processed += 1;
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
    ) -> Result<(), ContentError> {
        let markdown_text = std::fs::read_to_string(path)?;
        let (metadata, content_text) =
            Self::parse_frontmatter(&markdown_text, source.allowed_content_types)?;

        let resolved_category_id = metadata
            .category
            .clone()
            .unwrap_or_else(|| source.category_id.to_string());

        let final_content_text = if metadata.kind == ContentKind::Paper.as_str() {
            parser::load_paper_chapters(&markdown_text)?
        } else {
            content_text
        };

        let new_content = Self::create_content_from_metadata(
            &metadata,
            &final_content_text,
            source.source_id.to_string(),
            resolved_category_id,
        )?;

        // In dry_run mode, skip database operations but validate everything
        if dry_run {
            return Ok(());
        }

        let existing_content = self
            .content_repo
            .get_by_source_and_slug(&new_content.source_id, &new_content.slug)
            .await?;

        match existing_content {
            None => {
                let version_hash = Self::compute_version_hash(&new_content);
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
                .with_version_hash(version_hash)
                .with_links(new_content.links.clone());

                self.content_repo.create(&params).await?;
            },
            Some(existing) => {
                if override_existing {
                    let version_hash = Self::compute_version_hash(&new_content);
                    let update_params = UpdateContentParams::new(
                        existing.id.clone(),
                        new_content.title.clone(),
                        new_content.description.clone(),
                        new_content.body.clone(),
                    )
                    .with_keywords(new_content.keywords.clone())
                    .with_image(new_content.image.clone())
                    .with_version_hash(version_hash);
                    self.content_repo.update(&update_params).await?;
                }
            },
        }

        Ok(())
    }

    fn parse_frontmatter(
        markdown: &str,
        allowed_content_types: &[&str],
    ) -> Result<(ContentMetadata, String), ContentError> {
        let parts: Vec<&str> = markdown.splitn(3, "---").collect();

        if parts.len() < 3 {
            return Err(ContentError::Parse(
                "Invalid frontmatter format".to_string(),
            ));
        }

        let metadata: ContentMetadata = serde_yaml::from_str(parts[1])?;
        validate_content_metadata(&metadata, allowed_content_types)?;

        let content = parts[2].trim().to_string();

        Ok((metadata, content))
    }

    fn scan_markdown_files(
        dir: &Path,
        allowed_content_types: &[&str],
        recursive: bool,
    ) -> (Vec<std::path::PathBuf>, Vec<String>) {
        use walkdir::WalkDir;

        let mut files = Vec::new();
        let mut errors = Vec::new();

        let walker = if recursive {
            WalkDir::new(dir).min_depth(1)
        } else {
            WalkDir::new(dir).min_depth(1).max_depth(1)
        };

        for entry in walker.into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }

            let Some(ext) = entry.path().extension() else {
                continue;
            };

            if ext != "md" {
                continue;
            }

            match Self::validate_markdown_file(entry.path(), allowed_content_types) {
                Ok(()) => files.push(entry.path().to_path_buf()),
                Err(e) => errors.push(format!("{}: {}", entry.path().display(), e)),
            }
        }

        (files, errors)
    }

    fn validate_markdown_file(
        path: &Path,
        allowed_content_types: &[&str],
    ) -> Result<(), ContentError> {
        let markdown_text = std::fs::read_to_string(path)?;
        let (metadata, _) = Self::parse_frontmatter(&markdown_text, allowed_content_types)?;

        if metadata.kind == ContentKind::Paper.as_str() {
            parser::validate_paper_frontmatter(&markdown_text)?;
        }

        Ok(())
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
