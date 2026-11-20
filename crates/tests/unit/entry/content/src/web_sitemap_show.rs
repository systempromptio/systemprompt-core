//! Tests for `web sitemap show` over an explicit content config path.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;

use systemprompt_content_entry::cli::sitemap::show::{ShowArgs, execute_with_config_path};
use systemprompt_extension::cli::{CliConfig, OutputFormat};

fn cfg() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

const CONTENT_CONFIG: &str = r#"
content_sources:
  docs:
    path: content/docs
    source_id: docs
    category_id: docs
    enabled: true
    sitemap:
      enabled: true
      url_pattern: "/docs/{slug}"
      priority: 0.8
      changefreq: weekly
      parent_route:
        enabled: true
        url: "/docs"
        priority: 0.9
        changefreq: daily
  drafts:
    path: content/drafts
    source_id: drafts
    category_id: drafts
    enabled: false
"#;

#[test]
fn sitemap_show_lists_enabled_routes() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, CONTENT_CONFIG).unwrap();

    let out = execute_with_config_path(
        ShowArgs { preview: true },
        &cfg(),
        &config_path,
    )
    .unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["url"], "/docs");

    let err = execute_with_config_path(
        ShowArgs { preview: false },
        &cfg(),
        std::path::Path::new("/nonexistent/content-config.yaml"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Failed to read content config"));
}
