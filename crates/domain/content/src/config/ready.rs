use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::ContentRouting;
use walkdir::WalkDir;

use crate::models::ContentMetadata;
use crate::services::validate_content_metadata;
use crate::ContentError;

use super::validated::{ContentConfigValidated, ContentSourceConfigValidated};

#[derive(Debug, Clone)]
pub struct ContentReady {
    config: ContentConfigValidated,
    content_by_slug: HashMap<String, ParsedContent>,
    content_by_source: HashMap<SourceId, Vec<ParsedContent>>,
    stats: LoadStats,
}

#[derive(Debug, Clone)]
pub struct ParsedContent {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub author: String,
    pub published_at: DateTime<Utc>,
    pub keywords: String,
    pub kind: String,
    pub image: Option<String>,
    pub category_id: CategoryId,
    pub source_id: SourceId,
    pub version_hash: String,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct LoadStats {
    pub files_found: usize,
    pub files_loaded: usize,
    pub files_with_errors: usize,
    pub load_time_ms: u64,
    pub source_stats: HashMap<String, SourceLoadStats>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SourceLoadStats {
    pub files_found: usize,
    pub files_loaded: usize,
    pub errors: usize,
}

impl ContentReady {
    pub fn from_validated(config: ContentConfigValidated) -> Self {
        let start_time = std::time::Instant::now();
        let mut content_by_slug = HashMap::new();
        let mut content_by_source: HashMap<SourceId, Vec<ParsedContent>> = HashMap::new();
        let mut stats = LoadStats::default();

        for (source_name, source_config) in config.content_sources() {
            if !source_config.enabled {
                continue;
            }

            let mut source_stats = SourceLoadStats::default();

            let files = scan_markdown_files(&source_config.path, source_config.indexing.recursive);
            source_stats.files_found = files.len();
            stats.files_found += files.len();

            for file_path in files {
                match parse_content_file(&file_path, source_config) {
                    Ok(content) => {
                        let slug = content.slug.clone();
                        let source_id = content.source_id.clone();

                        content_by_source
                            .entry(source_id)
                            .or_default()
                            .push(content.clone());

                        content_by_slug.insert(slug, content);

                        source_stats.files_loaded += 1;
                        stats.files_loaded += 1;
                    },
                    Err(e) => {
                        tracing::warn!(
                            file = %file_path.display(),
                            error = %e,
                            "Failed to parse content file"
                        );
                        source_stats.errors += 1;
                        stats.files_with_errors += 1;
                    },
                }
            }

            stats.source_stats.insert(source_name.clone(), source_stats);
        }

        stats.load_time_ms = start_time.elapsed().as_millis() as u64;

        Self {
            config,
            content_by_slug,
            content_by_source,
            stats,
        }
    }

    pub const fn config(&self) -> &ContentConfigValidated {
        &self.config
    }

    pub const fn stats(&self) -> &LoadStats {
        &self.stats
    }

    pub fn get_by_slug(&self, slug: &str) -> Option<&ParsedContent> {
        self.content_by_slug.get(slug)
    }

    pub fn get_by_source(&self, source_id: &SourceId) -> Option<&Vec<ParsedContent>> {
        self.content_by_source.get(source_id)
    }

    pub fn all_content(&self) -> impl Iterator<Item = &ParsedContent> {
        self.content_by_slug.values()
    }

    pub fn content_count(&self) -> usize {
        self.content_by_slug.len()
    }
}

impl ContentRouting for ContentReady {
    fn is_html_page(&self, path: &str) -> bool {
        self.config.is_html_page(path)
    }

    fn determine_source(&self, path: &str) -> String {
        self.config.determine_source(path)
    }
}

fn scan_markdown_files(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let walker = if recursive {
        WalkDir::new(dir).min_depth(1)
    } else {
        WalkDir::new(dir).min_depth(1).max_depth(1)
    };

    walker
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn parse_content_file(
    file_path: &Path,
    source_config: &ContentSourceConfigValidated,
) -> Result<ParsedContent, ContentError> {
    let markdown_text = std::fs::read_to_string(file_path).map_err(ContentError::Io)?;

    let (metadata, body) = parse_frontmatter(&markdown_text)?;

    validate_content_metadata(&metadata)?;

    let published_at = parse_date(&metadata.published_at)?;

    let category_id = metadata.category.as_ref().map_or_else(
        || source_config.category_id.clone(),
        |c| CategoryId::new(c.clone()),
    );

    let version_hash = compute_version_hash(&metadata.title, &body, &metadata.description);

    Ok(ParsedContent {
        slug: metadata.slug,
        title: metadata.title,
        description: metadata.description,
        body,
        author: metadata.author,
        published_at,
        keywords: metadata.keywords,
        kind: metadata.kind,
        image: metadata.image,
        category_id,
        source_id: source_config.source_id.clone(),
        version_hash,
        file_path: file_path.to_path_buf(),
    })
}

fn parse_frontmatter(markdown: &str) -> Result<(ContentMetadata, String), ContentError> {
    let parts: Vec<&str> = markdown.splitn(3, "---").collect();

    if parts.len() < 3 {
        return Err(ContentError::Parse(
            "Invalid frontmatter format - missing '---' delimiters".to_string(),
        ));
    }

    let metadata: ContentMetadata = serde_yaml::from_str(parts[1]).map_err(ContentError::Yaml)?;

    Ok((metadata, parts[2].trim().to_string()))
}

fn parse_date(date_str: &str) -> Result<DateTime<Utc>, ContentError> {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| ContentError::Parse(format!("Invalid date '{}': {}", date_str, e)))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ContentError::Parse("Failed to create datetime".to_string()))?
        .and_local_timezone(Utc)
        .single()
        .ok_or_else(|| ContentError::Parse("Ambiguous timezone conversion".to_string()))
}

fn compute_version_hash(title: &str, body: &str, description: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(body.as_bytes());
    hasher.update(description.as_bytes());
    format!("{:x}", hasher.finalize())
}
