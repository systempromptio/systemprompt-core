//! Profile-backed tests for the `web content-types` command tree.
//!
//! The bootstrap fixture owns a tempdir profile, so the content config it
//! points at can be rewritten per test and the create/edit/delete commands
//! asserted against the file they actually mutate.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::web::content_types::{ContentTypesCommands, execute};
use systemprompt_cli::{CliConfig, OutputFormat, ScriptedPrompter};
use systemprompt_models::content_config::ContentConfigRaw;

const SEEDED: &str = r#"content_sources:
  blog:
    path: content/blog
    source_id: blog
    category_id: blog
    enabled: true
    description: Blog posts
    sitemap:
      enabled: true
      url_pattern: /blog/{slug}
      priority: 0.7
      changefreq: weekly
  docs:
    path: content/docs
    source_id: docs
    category_id: docs
    enabled: false
categories:
  blog:
    name: blog
  docs:
    name: docs
"#;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: ContentTypesCommands,
}

fn parse(args: &[&str]) -> ContentTypesCommands {
    Harness::try_parse_from(std::iter::once("content-types").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn config() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

fn config_path() -> String {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    boot.services_path
        .join("content/config.yaml")
        .to_string_lossy()
        .to_string()
}

fn seed() -> String {
    let path = config_path();
    std::fs::write(&path, SEEDED).unwrap();
    path
}

fn reload(path: &str) -> ContentConfigRaw {
    serde_yaml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run(args: &[&str]) -> anyhow::Result<()> {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    execute(parse(args), &prompter, &config())
}

#[test]
fn list_and_show_render_the_seeded_types() {
    seed();

    run(&["list"]).unwrap();
    run(&["show", "blog"]).unwrap();
    run(&["show", "docs"]).unwrap();
}

#[test]
fn show_rejects_an_unknown_type() {
    seed();

    let err = run(&["show", "ghost"]).unwrap_err();
    assert!(err.to_string().contains("show content type"));
    assert!(format!("{err:#}").contains("'ghost' not found"));
}

#[test]
fn create_writes_a_new_source_with_a_sitemap() {
    let path = seed();

    run(&[
        "create",
        "--name",
        "guides",
        "--path",
        "content/guides",
        "--source-id",
        "guides",
        "--category-id",
        "blog",
        "--description",
        "Guides",
        "--enabled",
        "--url-pattern",
        "/guides/{slug}",
        "--priority",
        "0.9",
        "--changefreq",
        "daily",
    ])
    .unwrap();

    let config = reload(&path);
    let created = config.content_sources.get("guides").unwrap();
    assert_eq!(created.path, "content/guides");
    assert!(created.enabled);
    let sitemap = created.sitemap.as_ref().unwrap();
    assert_eq!(sitemap.url_pattern, "/guides/{slug}");
    assert_eq!(sitemap.changefreq, "daily");
    assert!(config.content_sources.contains_key("blog"));
}

#[test]
fn create_rejects_an_unknown_category() {
    let path = seed();

    let err = run(&[
        "create",
        "--name",
        "orphan",
        "--path",
        "content/orphan",
        "--source-id",
        "orphan",
        "--category-id",
        "nosuchcategory",
    ])
    .unwrap_err();

    assert!(format!("{err:#}").contains("Category 'nosuchcategory' not found"));
    assert!(!reload(&path).content_sources.contains_key("orphan"));
}

#[test]
fn edit_applies_flags_and_set_values() {
    let path = seed();

    run(&[
        "edit",
        "docs",
        "--enable",
        "--path",
        "content/documentation",
        "--set",
        "description=Reference material",
    ])
    .unwrap();

    let config = reload(&path);
    let docs = config.content_sources.get("docs").unwrap();
    assert!(docs.enabled);
    assert_eq!(docs.path, "content/documentation");
    assert_eq!(docs.description, "Reference material");
}

#[test]
fn edit_updates_sitemap_fields() {
    let path = seed();

    run(&[
        "edit",
        "blog",
        "--url-pattern",
        "/posts/{slug}",
        "--priority",
        "0.4",
        "--changefreq",
        "monthly",
    ])
    .unwrap();

    let sitemap = reload(&path)
        .content_sources
        .get("blog")
        .unwrap()
        .sitemap
        .clone()
        .unwrap();
    assert_eq!(sitemap.url_pattern, "/posts/{slug}");
    assert_eq!(sitemap.priority, 0.4);
    assert_eq!(sitemap.changefreq, "monthly");
}

#[test]
fn edit_rejects_an_unknown_type() {
    seed();

    let err = run(&["edit", "ghost", "--enable"]).unwrap_err();
    assert!(format!("{err:#}").contains("'ghost' not found"));
}

#[test]
fn delete_removes_the_source_from_the_config() {
    let path = seed();

    run(&["delete", "docs", "--yes"]).unwrap();

    let config = reload(&path);
    assert!(!config.content_sources.contains_key("docs"));
    assert!(config.content_sources.contains_key("blog"));
}

#[test]
fn delete_rejects_an_unknown_type_before_confirming() {
    let path = seed();

    let err = run(&["delete", "ghost", "--yes"]).unwrap_err();
    assert!(format!("{err:#}").contains("'ghost' not found"));
    assert_eq!(reload(&path).content_sources.len(), 2);
}

#[test]
fn delete_without_yes_is_refused_in_non_interactive_mode() {
    let path = seed();

    let err = run(&["delete", "docs"]).unwrap_err();
    assert!(format!("{err:#}").contains("--yes is required"));
    assert!(reload(&path).content_sources.contains_key("docs"));
}
