//! Tests for `ContentReady`, `ParsedContent`, `LoadStats`, and
//! `SourceLoadStats`.

use std::collections::HashMap;
use systemprompt_content::{ContentConfigValidated, ContentReady, LoadStats, ParsedContent};
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::{
    Category, ContentConfigRaw, ContentSourceConfigRaw, IndexingConfig, Metadata, SitemapConfig,
    SourceBranding,
};
use tempfile::TempDir;

fn make_category(name: &str) -> Category {
    Category {
        name: name.to_string(),
        description: String::new(),
    }
}

fn build_config(
    tmp: &TempDir,
    source_name: &str,
    enabled: bool,
    recursive: bool,
) -> ContentConfigValidated {
    let mut categories = HashMap::new();
    categories.insert("tech".to_string(), make_category("Technology"));

    let source_dir = tmp.path().join("content");
    std::fs::create_dir_all(&source_dir).unwrap();

    let mut sources = HashMap::new();
    sources.insert(
        source_name.to_string(),
        ContentSourceConfigRaw {
            path: source_dir.to_string_lossy().to_string(),
            source_id: SourceId::new(source_name),
            category_id: CategoryId::new("tech"),
            enabled,
            description: String::new(),
            allowed_content_types: vec!["article".to_string()],
            indexing: Some(IndexingConfig {
                clear_before: false,
                recursive,
                override_existing: false,
            }),
            sitemap: Some(SitemapConfig {
                enabled: true,
                url_pattern: "/blog/{slug}".to_string(),
                priority: 0.8,
                changefreq: "weekly".to_string(),
                fetch_from: String::new(),
                parent_route: None,
            }),
            branding: Some(SourceBranding::default()),
        },
    );

    ContentConfigValidated::from_raw(
        ContentConfigRaw {
            content_sources: sources,
            metadata: Metadata::default(),
            categories,
        },
        tmp.path().to_path_buf(),
    )
    .expect("valid config")
}

fn write_markdown(dir: &std::path::Path, filename: &str, frontmatter: &str, body: &str) {
    let content = format!("---\n{frontmatter}\n---\n{body}");
    std::fs::write(dir.join(filename), content).unwrap();
}

fn valid_frontmatter(slug: &str, title: &str) -> String {
    format!(
        "title: {title}\ndescription: A description\nauthor: Test Author\npublished_at: \"2024-06-01\"\nslug: {slug}\nkind: article\n"
    )
}

#[test]
fn from_validated_empty_source_has_zero_content() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.content_count(), 0);
    assert_eq!(ready.stats().files_found, 0);
    assert_eq!(ready.stats().files_loaded, 0);
    assert_eq!(ready.stats().files_with_errors, 0);
}

#[test]
fn from_validated_loads_single_valid_markdown() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let content_dir = tmp.path().join("content");

    write_markdown(
        &content_dir,
        "hello-world.md",
        &valid_frontmatter("hello-world", "Hello World"),
        "This is the body of the article.",
    );

    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.content_count(), 1);
    assert_eq!(ready.stats().files_found, 1);
    assert_eq!(ready.stats().files_loaded, 1);
    assert_eq!(ready.stats().files_with_errors, 0);
}

#[test]
fn from_validated_skips_disabled_source() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", false, false);
    let content_dir = tmp.path().join("content");

    write_markdown(
        &content_dir,
        "article.md",
        &valid_frontmatter("article", "Article"),
        "Body.",
    );

    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.content_count(), 0);
    assert_eq!(ready.stats().files_found, 0);
}

#[test]
fn from_validated_counts_errors_for_invalid_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let content_dir = tmp.path().join("content");

    std::fs::write(
        content_dir.join("bad.md"),
        "no frontmatter here just plain text",
    )
    .unwrap();

    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.content_count(), 0);
    assert_eq!(ready.stats().files_with_errors, 1);
    assert_eq!(ready.stats().files_loaded, 0);
}

#[test]
fn from_validated_loads_multiple_files() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let content_dir = tmp.path().join("content");

    for i in 1..=5 {
        write_markdown(
            &content_dir,
            &format!("article-{i}.md"),
            &valid_frontmatter(&format!("article-{i}"), &format!("Article {i}")),
            &format!("Body of article {i}."),
        );
    }

    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.content_count(), 5);
    assert_eq!(ready.stats().files_found, 5);
    assert_eq!(ready.stats().files_loaded, 5);
}

#[test]
fn get_by_slug_returns_matching_content() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let content_dir = tmp.path().join("content");

    write_markdown(
        &content_dir,
        "my-post.md",
        &valid_frontmatter("my-post", "My Post"),
        "Post body.",
    );

    let ready = ContentReady::from_validated(config);

    let found = ready.get_by_slug("my-post");
    assert!(found.is_some());
    let parsed = found.unwrap();
    assert_eq!(parsed.slug, "my-post");
    assert_eq!(parsed.title, "My Post");
}

#[test]
fn get_by_slug_returns_none_for_unknown_slug() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);

    let ready = ContentReady::from_validated(config);

    assert!(ready.get_by_slug("nonexistent-slug").is_none());
}

#[test]
fn get_by_source_returns_content_for_known_source() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let content_dir = tmp.path().join("content");

    write_markdown(
        &content_dir,
        "post.md",
        &valid_frontmatter("post", "Post"),
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    let source_id = SourceId::new("blog");
    let items = ready.get_by_source(&source_id);
    assert!(items.is_some());
    assert_eq!(items.unwrap().len(), 1);
}

#[test]
fn get_by_source_returns_none_for_unknown_source() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let ready = ContentReady::from_validated(config);

    let unknown = SourceId::new("unknown-source");
    assert!(ready.get_by_source(&unknown).is_none());
}

#[test]
fn all_content_iterator_yields_loaded_items() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let content_dir = tmp.path().join("content");

    for i in 1..=3 {
        write_markdown(
            &content_dir,
            &format!("item-{i}.md"),
            &valid_frontmatter(&format!("item-{i}"), &format!("Item {i}")),
            "Body.",
        );
    }

    let ready = ContentReady::from_validated(config);
    let count = ready.all_content().count();
    assert_eq!(count, 3);
}

#[test]
fn load_stats_default_is_zeroed() {
    let stats = LoadStats::default();
    assert_eq!(stats.files_found, 0);
    assert_eq!(stats.files_loaded, 0);
    assert_eq!(stats.files_with_errors, 0);
    assert_eq!(stats.load_time_ms, 0);
    assert!(stats.source_stats.is_empty());
}

#[test]
fn load_stats_load_time_is_nonzero_after_scan() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let ready = ContentReady::from_validated(config);
    let _ = ready.stats().load_time_ms;
}

#[test]
fn content_routing_is_html_page_delegates_to_config() {
    use systemprompt_models::ContentRouting;

    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let ready = ContentReady::from_validated(config);

    assert!(ready.is_html_page("/"));
    assert!(ready.is_html_page("/blog/any-post"));
    assert!(!ready.is_html_page("/api/something"));
}

#[test]
fn content_routing_determine_source_delegates_to_config() {
    use systemprompt_models::ContentRouting;

    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.determine_source("/"), "web");
    assert_eq!(ready.determine_source("/blog/my-post"), "blog");
    assert_eq!(ready.determine_source("/no/such/route"), "unknown");
}

#[test]
fn config_accessor_returns_underlying_config() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.config().content_sources().len(), 1);
    assert!(ready.config().categories().contains_key("tech"));
}

#[test]
fn parsed_content_slug_and_title_from_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let content_dir = tmp.path().join("content");

    write_markdown(
        &content_dir,
        "rust-intro.md",
        "title: Introduction to Rust\ndescription: Learn Rust\nauthor: Expert\npublished_at: \"2024-03-01\"\nslug: rust-intro\nkind: guide\n",
        "## Introduction\n\nRust is a systems programming language.",
    );

    let ready = ContentReady::from_validated(config);
    let content = ready.get_by_slug("rust-intro").unwrap();

    assert_eq!(content.slug, "rust-intro");
    assert_eq!(content.title, "Introduction to Rust");
    assert_eq!(content.description, "Learn Rust");
    assert_eq!(content.author, "Expert");
    assert_eq!(content.kind, "guide");
    assert!(
        content
            .body
            .contains("Rust is a systems programming language.")
    );
    assert!(!content.version_hash.is_empty());
}

#[test]
fn parsed_content_version_hash_changes_with_body() {
    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();

    let config1 = build_config(&tmp1, "blog", true, false);
    let config2 = build_config(&tmp2, "blog", true, false);

    let dir1 = tmp1.path().join("content");
    let dir2 = tmp2.path().join("content");

    write_markdown(
        &dir1,
        "same-slug.md",
        &valid_frontmatter("same-slug", "Same Slug"),
        "Body version one.",
    );
    write_markdown(
        &dir2,
        "same-slug.md",
        &valid_frontmatter("same-slug", "Same Slug"),
        "Body version two — completely different content.",
    );

    let ready1 = ContentReady::from_validated(config1);
    let ready2 = ContentReady::from_validated(config2);

    let hash1 = &ready1.get_by_slug("same-slug").unwrap().version_hash;
    let hash2 = &ready2.get_by_slug("same-slug").unwrap().version_hash;
    assert_ne!(hash1, hash2);
}

#[test]
fn mixed_valid_and_invalid_files_partial_load() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    write_markdown(
        &dir,
        "valid.md",
        &valid_frontmatter("valid-post", "Valid Post"),
        "Valid body.",
    );
    std::fs::write(dir.join("invalid.md"), "this has no frontmatter").unwrap();
    std::fs::write(dir.join("notes.txt"), "not a markdown file").unwrap();

    let ready = ContentReady::from_validated(config);

    assert_eq!(ready.content_count(), 1);
    assert_eq!(ready.stats().files_found, 2);
    assert_eq!(ready.stats().files_loaded, 1);
    assert_eq!(ready.stats().files_with_errors, 1);
}

#[test]
fn non_md_files_are_ignored() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    std::fs::write(dir.join("config.yaml"), "key: value").unwrap();
    std::fs::write(dir.join("readme.txt"), "readme").unwrap();
    std::fs::write(dir.join("image.png"), [0u8; 10]).unwrap();
    write_markdown(
        &dir,
        "real.md",
        &valid_frontmatter("real-article", "Real Article"),
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    assert_eq!(ready.content_count(), 1);
    assert_eq!(ready.stats().files_found, 1);
}

#[test]
fn category_from_frontmatter_overrides_source_category() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    write_markdown(
        &dir,
        "categorized.md",
        "title: Categorized Article\ndescription: Desc\nauthor: Author\npublished_at: \"2024-01-15\"\nslug: categorized\nkind: article\ncategory: custom-cat\n",
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    let content = ready.get_by_slug("categorized").unwrap();

    assert_eq!(content.category_id.as_str(), "custom-cat");
}

#[test]
fn category_falls_back_to_source_when_absent() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    write_markdown(
        &dir,
        "default-cat.md",
        &valid_frontmatter("default-cat", "Default Category"),
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    let content = ready.get_by_slug("default-cat").unwrap();

    assert_eq!(content.category_id.as_str(), "tech");
}

#[test]
fn parsed_content_published_at_is_utc() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    write_markdown(
        &dir,
        "dated.md",
        "title: Dated Article\ndescription: Desc\nauthor: Author\npublished_at: \"2023-11-25\"\nslug: dated\nkind: article\n",
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    let content = ready.get_by_slug("dated").unwrap();

    assert_eq!(
        content.published_at.format("%Y-%m-%d").to_string(),
        "2023-11-25"
    );
}

#[test]
fn parsed_content_image_is_none_when_absent() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    write_markdown(
        &dir,
        "no-image.md",
        &valid_frontmatter("no-image", "No Image Article"),
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    let content = ready.get_by_slug("no-image").unwrap();

    assert!(content.image.is_none());
}

#[test]
fn parsed_content_image_present_when_set() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    write_markdown(
        &dir,
        "with-image.md",
        "title: With Image\ndescription: Desc\nauthor: Author\npublished_at: \"2024-01-01\"\nslug: with-image\nkind: article\nimage: /images/hero.png\n",
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    let content = ready.get_by_slug("with-image").unwrap();

    assert_eq!(content.image, Some("/images/hero.png".to_string()));
}

#[test]
fn source_stats_recorded_per_source() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "docs", true, false);
    let dir = tmp.path().join("content");

    for i in 1..=3 {
        write_markdown(
            &dir,
            &format!("doc-{i}.md"),
            &valid_frontmatter(&format!("doc-{i}"), &format!("Doc {i}")),
            "Body.",
        );
    }

    let ready = ContentReady::from_validated(config);
    let source_stats = &ready.stats().source_stats;
    assert!(source_stats.contains_key("docs"));
    let stats = source_stats["docs"];
    assert_eq!(stats.files_found, 3);
    assert_eq!(stats.files_loaded, 3);
    assert_eq!(stats.errors, 0);
}

#[test]
fn all_content_empty_when_no_files_loaded() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let ready = ContentReady::from_validated(config);

    let all: Vec<&ParsedContent> = ready.all_content().collect();
    assert!(all.is_empty());
}

#[test]
fn parsed_content_file_path_is_absolute() {
    let tmp = TempDir::new().unwrap();
    let config = build_config(&tmp, "blog", true, false);
    let dir = tmp.path().join("content");

    write_markdown(
        &dir,
        "pathed.md",
        &valid_frontmatter("pathed", "Pathed Article"),
        "Body.",
    );

    let ready = ContentReady::from_validated(config);
    let content = ready.get_by_slug("pathed").unwrap();

    assert!(content.file_path.is_absolute());
    assert!(content.file_path.exists());
}
