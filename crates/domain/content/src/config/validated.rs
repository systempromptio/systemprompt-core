use std::collections::HashMap;
use std::path::{Path, PathBuf};
use systemprompt_models::{
    Category, ContentConfigError, ContentConfigErrors, ContentConfigRaw, ContentRouting,
    ContentSourceConfigRaw, IndexingConfig, Metadata, SitemapConfig, SourceBranding,
};

#[derive(Debug, Clone)]
pub struct ContentConfigValidated {
    content_sources: HashMap<String, ContentSourceConfigValidated>,
    metadata: Metadata,
    categories: HashMap<String, Category>,
    base_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ContentSourceConfigValidated {
    pub path: PathBuf,
    pub source_id: String,
    pub category_id: String,
    pub enabled: bool,
    pub description: String,
    pub allowed_content_types: Vec<String>,
    pub indexing: IndexingConfig,
    pub sitemap: Option<SitemapConfig>,
    pub branding: Option<SourceBranding>,
}

pub type ValidationResult = Result<ContentConfigValidated, ContentConfigErrors>;

impl ContentConfigValidated {
    pub fn from_raw(raw: ContentConfigRaw, base_path: PathBuf) -> ValidationResult {
        let mut errors = ContentConfigErrors::new();

        let categories = validate_categories(&raw.categories, &mut errors);
        let content_sources = validate_sources(&raw, &categories, &base_path, &mut errors);

        errors.into_result(Self {
            content_sources,
            metadata: raw.metadata,
            categories,
            base_path,
        })
    }

    pub const fn content_sources(&self) -> &HashMap<String, ContentSourceConfigValidated> {
        &self.content_sources
    }

    pub const fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub const fn categories(&self) -> &HashMap<String, Category> {
        &self.categories
    }

    pub const fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    pub fn is_html_page(&self, path: &str) -> bool {
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

    fn matches_url_pattern(pattern: &str, path: &str) -> bool {
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

    pub fn determine_source(&self, path: &str) -> String {
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

impl ContentRouting for ContentConfigValidated {
    fn is_html_page(&self, path: &str) -> bool {
        ContentConfigValidated::is_html_page(self, path)
    }

    fn determine_source(&self, path: &str) -> String {
        ContentConfigValidated::determine_source(self, path)
    }
}

fn validate_categories(
    raw: &HashMap<String, Category>,
    errors: &mut ContentConfigErrors,
) -> HashMap<String, Category> {
    let mut validated = HashMap::new();

    for (id, cat) in raw {
        if cat.name.is_empty() {
            errors.push(ContentConfigError::Validation {
                field: format!("categories.{id}.name"),
                message: "Category name cannot be empty".to_string(),
                suggestion: Some("Provide a non-empty name".to_string()),
            });
            continue;
        }
        validated.insert(id.clone(), cat.clone());
    }

    validated
}

fn validate_sources(
    raw: &ContentConfigRaw,
    categories: &HashMap<String, Category>,
    base_path: &Path,
    errors: &mut ContentConfigErrors,
) -> HashMap<String, ContentSourceConfigValidated> {
    let mut validated = HashMap::new();

    for (name, source) in &raw.content_sources {
        if let Some(validated_source) =
            validate_single_source(name, source, categories, base_path, errors)
        {
            validated.insert(name.clone(), validated_source);
        }
    }

    validated
}

fn validate_single_source(
    name: &str,
    source: &ContentSourceConfigRaw,
    categories: &HashMap<String, Category>,
    base_path: &Path,
    errors: &mut ContentConfigErrors,
) -> Option<ContentSourceConfigValidated> {
    let field_prefix = format!("content_sources.{name}");

    if source.path.is_empty() {
        errors.push(ContentConfigError::Validation {
            field: format!("{field_prefix}.path"),
            message: "Source path is required".to_string(),
            suggestion: Some("Add a path to the content directory".to_string()),
        });
        return None;
    }

    if source.source_id.as_str().is_empty() {
        errors.push(ContentConfigError::Validation {
            field: format!("{field_prefix}.source_id"),
            message: "source_id is required".to_string(),
            suggestion: Some("Add a unique source_id".to_string()),
        });
        return None;
    }

    if source.category_id.as_str().is_empty() {
        errors.push(ContentConfigError::Validation {
            field: format!("{field_prefix}.category_id"),
            message: "category_id is required".to_string(),
            suggestion: Some("Add a category_id that references a defined category".to_string()),
        });
        return None;
    }

    if !categories.contains_key(source.category_id.as_str()) {
        errors.push(ContentConfigError::Validation {
            field: format!("{field_prefix}.category_id"),
            message: format!("Referenced category '{}' not found", source.category_id),
            suggestion: Some("Add this category to the categories section".to_string()),
        });
    }

    let resolved_path = if source.path.starts_with('/') {
        PathBuf::from(&source.path)
    } else {
        base_path.join(&source.path)
    };

    let Ok(canonical_path) = std::fs::canonicalize(&resolved_path) else {
        errors.push(ContentConfigError::Validation {
            field: format!("{field_prefix}.path"),
            message: "Content source directory does not exist".to_string(),
            suggestion: Some("Create the directory or fix the path".to_string()),
        });
        return None;
    };

    if source.enabled && source.allowed_content_types.is_empty() {
        errors.push(ContentConfigError::Validation {
            field: format!("{field_prefix}.allowed_content_types"),
            message: "Enabled source must have at least one allowed_content_type".to_string(),
            suggestion: Some("Add content types like 'article', 'paper', 'guide'".to_string()),
        });
    }

    Some(ContentSourceConfigValidated {
        path: canonical_path,
        source_id: source.source_id.to_string(),
        category_id: source.category_id.to_string(),
        enabled: source.enabled,
        description: source.description.clone(),
        allowed_content_types: source.allowed_content_types.clone(),
        indexing: source.indexing.unwrap_or_default(),
        sitemap: source.sitemap.clone(),
        branding: source.branding.clone(),
    })
}
