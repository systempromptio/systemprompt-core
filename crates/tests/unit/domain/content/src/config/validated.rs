//! Unit tests for `ContentConfigValidated` URL-routing helpers
//! (`is_html_page`, `determine_source`, `resolve_slug`).
//!
//! Each fixture builds a temporary on-disk source directory because the
//! validator canonicalises paths.

use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_content::ContentConfigValidated;
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::{
    Category, ContentConfigRaw, ContentSourceConfigRaw, IndexingConfig, Metadata, SitemapConfig,
    SourceBranding,
};
use tempfile::TempDir;

fn build_raw(tmp: &TempDir, name: &str, pattern: &str, enabled: bool) -> ContentConfigRaw {
    let mut categories = HashMap::new();
    categories.insert(
        "tech".to_string(),
        Category {
            name: "Technology".to_string(),
            description: String::new(),
        },
    );

    let source_dir = tmp.path().join("content");
    std::fs::create_dir_all(&source_dir).expect("create content dir");

    let mut sources = HashMap::new();
    sources.insert(
        name.to_string(),
        ContentSourceConfigRaw {
            path: source_dir.to_string_lossy().to_string(),
            source_id: SourceId::new(name),
            category_id: CategoryId::new("tech"),
            enabled,
            description: String::new(),
            allowed_content_types: vec!["article".to_string()],
            indexing: Some(IndexingConfig::default()),
            sitemap: Some(SitemapConfig {
                enabled: true,
                url_pattern: pattern.to_string(),
                priority: 0.8,
                changefreq: "weekly".to_string(),
                fetch_from: String::new(),
                parent_route: None,
            }),
            branding: Some(SourceBranding::default()),
        },
    );

    ContentConfigRaw {
        content_sources: sources,
        metadata: Metadata::default(),
        categories,
    }
}

#[test]
fn test_from_raw_with_existing_dir_succeeds() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated =
        ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).expect("valid config");
    assert_eq!(validated.content_sources().len(), 1);
    assert!(validated.categories().contains_key("tech"));
}

#[test]
fn test_from_raw_missing_path_fails() {
    let mut categories = HashMap::new();
    categories.insert(
        "tech".to_string(),
        Category {
            name: "Tech".to_string(),
            description: String::new(),
        },
    );
    let mut sources = HashMap::new();
    sources.insert(
        "blog".to_string(),
        ContentSourceConfigRaw {
            path: String::new(),
            source_id: SourceId::new("blog"),
            category_id: CategoryId::new("tech"),
            enabled: true,
            description: String::new(),
            allowed_content_types: vec![],
            indexing: None,
            sitemap: None,
            branding: None,
        },
    );
    let raw = ContentConfigRaw {
        content_sources: sources,
        metadata: Metadata::default(),
        categories,
    };

    let result = ContentConfigValidated::from_raw(raw, PathBuf::from("/tmp"));
    result.unwrap_err();
}

#[test]
fn test_from_raw_unknown_category_fails() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("content")).unwrap();
    let mut sources = HashMap::new();
    sources.insert(
        "blog".to_string(),
        ContentSourceConfigRaw {
            path: tmp.path().join("content").to_string_lossy().to_string(),
            source_id: SourceId::new("blog"),
            category_id: CategoryId::new("ghost-category"),
            enabled: true,
            description: String::new(),
            allowed_content_types: vec![],
            indexing: None,
            sitemap: None,
            branding: None,
        },
    );
    let raw = ContentConfigRaw {
        content_sources: sources,
        metadata: Metadata::default(),
        categories: HashMap::new(),
    };
    let result = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf());
    result.unwrap_err();
}

#[test]
fn test_is_html_page_root_always_true() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert!(validated.is_html_page("/"));
}

#[test]
fn test_is_html_page_matching_pattern() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert!(validated.is_html_page("/blog/hello-world"));
}

#[test]
fn test_is_html_page_non_matching() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert!(!validated.is_html_page("/api/users"));
    assert!(!validated.is_html_page("/blog/a/b"));
}

#[test]
fn test_is_html_page_disabled_source() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", false);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert!(!validated.is_html_page("/blog/hello"));
}

#[test]
fn test_determine_source_root_is_web() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert_eq!(validated.determine_source("/"), "web");
}

#[test]
fn test_determine_source_match() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert_eq!(validated.determine_source("/blog/hello"), "blog");
}

#[test]
fn test_determine_source_unknown_path() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert_eq!(validated.determine_source("/no/such/route"), "unknown");
}

#[test]
fn test_resolve_slug_with_slug_pattern() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert_eq!(
        validated.resolve_slug("/blog/my-post"),
        Some("my-post".to_string())
    );
}

#[test]
fn test_resolve_slug_trims_trailing_slash() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert_eq!(
        validated.resolve_slug("/blog/my-post/"),
        Some("my-post".to_string())
    );
}

#[test]
fn test_resolve_slug_strips_query_and_fragment() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert_eq!(
        validated.resolve_slug("/blog/my-post?utm=1"),
        Some("my-post".to_string())
    );
    assert_eq!(
        validated.resolve_slug("/blog/my-post#section"),
        Some("my-post".to_string())
    );
}

#[test]
fn test_resolve_slug_no_match() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert!(validated.resolve_slug("/no-such/route").is_none());
}

#[test]
fn test_metadata_and_base_path_accessors() {
    let tmp = TempDir::new().unwrap();
    let raw = build_raw(&tmp, "blog", "/blog/{slug}", true);
    let validated = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap();
    assert_eq!(validated.base_path(), &tmp.path().to_path_buf());
    let _ = validated.metadata();
}

fn raw_with_source(
    categories: HashMap<String, Category>,
    source: ContentSourceConfigRaw,
) -> ContentConfigRaw {
    let mut sources = HashMap::new();
    sources.insert("blog".to_string(), source);
    ContentConfigRaw {
        content_sources: sources,
        metadata: Metadata::default(),
        categories,
    }
}

fn tech_categories() -> HashMap<String, Category> {
    let mut categories = HashMap::new();
    categories.insert(
        "tech".to_string(),
        Category {
            name: "Technology".to_string(),
            description: String::new(),
        },
    );
    categories
}

fn source(path: &str, source_id: &str, category_id: &str) -> ContentSourceConfigRaw {
    ContentSourceConfigRaw {
        path: path.to_string(),
        source_id: SourceId::new(source_id),
        category_id: CategoryId::new(category_id),
        enabled: true,
        description: String::new(),
        allowed_content_types: vec![],
        indexing: None,
        sitemap: None,
        branding: None,
    }
}

#[test]
fn test_empty_category_name_is_dropped_and_fails_validation() {
    let mut categories = HashMap::new();
    categories.insert(
        "tech".to_string(),
        Category {
            name: String::new(),
            description: String::new(),
        },
    );
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("content");
    std::fs::create_dir_all(&dir).unwrap();
    let raw = raw_with_source(
        categories,
        source(&dir.to_string_lossy(), "blog", "tech"),
    );

    let err = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap_err();
    assert!(
        err.to_string().contains("categories.tech.name"),
        "error must name the empty-name category: {err}"
    );
}

#[test]
fn test_empty_source_id_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("content");
    std::fs::create_dir_all(&dir).unwrap();
    let raw = raw_with_source(
        tech_categories(),
        source(&dir.to_string_lossy(), "", "tech"),
    );

    let err = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap_err();
    assert!(
        err.to_string().contains("source_id"),
        "error must name source_id: {err}"
    );
}

#[test]
fn test_empty_category_id_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("content");
    std::fs::create_dir_all(&dir).unwrap();
    let raw = raw_with_source(
        tech_categories(),
        source(&dir.to_string_lossy(), "blog", ""),
    );

    let err = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap_err();
    assert!(
        err.to_string().contains("category_id"),
        "error must name category_id: {err}"
    );
}

#[test]
fn test_nonexistent_source_directory_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("does-not-exist");
    let raw = raw_with_source(
        tech_categories(),
        source(&missing.to_string_lossy(), "blog", "tech"),
    );

    let err = ContentConfigValidated::from_raw(raw, tmp.path().to_path_buf()).unwrap_err();
    assert!(
        err.to_string().contains("does not exist")
            || err.to_string().contains("content_sources.blog.path"),
        "error must flag the missing directory: {err}"
    );
}
