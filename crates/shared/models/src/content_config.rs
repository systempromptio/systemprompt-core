use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::{CategoryId, SourceId};
use thiserror::Error;

pub trait ContentRouting: Send + Sync {
    fn is_html_page(&self, path: &str) -> bool;
    fn determine_source(&self, path: &str) -> String;
}

impl<T: ContentRouting + ?Sized> ContentRouting for Arc<T> {
    fn is_html_page(&self, path: &str) -> bool {
        (**self).is_html_page(path)
    }

    fn determine_source(&self, path: &str) -> String {
        (**self).determine_source(path)
    }
}

#[derive(Debug, Clone, Error)]
pub enum ContentConfigError {
    #[error("IO error reading {path}: {message}")]
    Io { path: PathBuf, message: String },

    #[error("YAML parse error in {path}: {message}")]
    Parse { path: PathBuf, message: String },

    #[error("Validation error in {field}: {message}")]
    Validation {
        field: String,
        message: String,
        suggestion: Option<String>,
    },
}

#[derive(Debug, Default)]
pub struct ContentConfigErrors {
    errors: Vec<ContentConfigError>,
}

impl ContentConfigErrors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, error: ContentConfigError) {
        self.errors.push(error);
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn errors(&self) -> &[ContentConfigError] {
        &self.errors
    }

    pub fn into_result<T>(self, value: T) -> Result<T, Self> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(self)
        }
    }
}

impl std::fmt::Display for ContentConfigErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, error) in self.errors.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "  - {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ContentConfigErrors {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentConfigRaw {
    #[serde(default)]
    pub content_sources: HashMap<String, ContentSourceConfigRaw>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default)]
    pub categories: HashMap<String, Category>,
}

impl ContentConfigRaw {
    pub fn matches_url_pattern(pattern: &str, path: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
        let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if pattern_parts.len() != path_parts.len() {
            return false;
        }

        pattern_parts
            .iter()
            .zip(path_parts.iter())
            .all(|(pattern_part, path_part)| *pattern_part == "{slug}" || pattern_part == path_part)
    }
}

impl ContentRouting for ContentConfigRaw {
    fn is_html_page(&self, path: &str) -> bool {
        if path == "/" {
            return true;
        }

        self.content_sources
            .values()
            .filter(|source| source.enabled)
            .filter_map(|source| source.sitemap.as_ref())
            .filter(|sitemap| sitemap.enabled)
            .any(|sitemap| Self::matches_url_pattern(&sitemap.url_pattern, path))
    }

    fn determine_source(&self, path: &str) -> String {
        if path == "/" {
            return "web".to_string();
        }

        self.content_sources
            .iter()
            .filter(|(_, source)| source.enabled)
            .find_map(|(name, source)| {
                source.sitemap.as_ref().and_then(|sitemap| {
                    (sitemap.enabled && Self::matches_url_pattern(&sitemap.url_pattern, path))
                        .then(|| name.clone())
                })
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSourceConfigRaw {
    pub path: String,
    pub source_id: SourceId,
    pub category_id: CategoryId,
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub allowed_content_types: Vec<String>,
    #[serde(default)]
    pub indexing: Option<IndexingConfig>,
    #[serde(default)]
    pub sitemap: Option<SitemapConfig>,
    #[serde(default)]
    pub branding: Option<SourceBranding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceBranding {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub keywords: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct IndexingConfig {
    #[serde(default)]
    pub clear_before: bool,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub override_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitemapConfig {
    pub enabled: bool,
    pub url_pattern: String,
    pub priority: f32,
    pub changefreq: String,
    #[serde(default)]
    pub fetch_from: String,
    #[serde(default)]
    pub parent_route: Option<ParentRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentRoute {
    pub enabled: bool,
    pub url: String,
    pub priority: f32,
    pub changefreq: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    #[serde(default)]
    pub default_author: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub structured_data: StructuredData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StructuredData {
    #[serde(default)]
    pub organization: OrganizationData,
    #[serde(default)]
    pub article: ArticleDefaults,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrganizationData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub logo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArticleDefaults {
    #[serde(default)]
    pub article_type: String,
    #[serde(default)]
    pub article_section: String,
    #[serde(default)]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Category {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}
