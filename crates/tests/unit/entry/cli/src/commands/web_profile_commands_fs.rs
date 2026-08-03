//! Profile-backed tests for `web assets` and `web validate`.
//!
//! Both resolve their inputs from the bootstrapped profile, so the fixture's
//! tempdir tree is populated per test and the commands asserted against what
//! they read.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::web::{self, WebCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: WebCommands,
}

fn parse(args: &[&str]) -> WebCommands {
    Harness::try_parse_from(std::iter::once("web").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

fn run(args: &[&str]) -> anyhow::Result<()> {
    web::execute(parse(args), &ctx())
}

fn services_root() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    boot.services_path.clone()
}

fn assets_dir() -> PathBuf {
    let dir = services_root().join("web/assets");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn content_config_path() -> PathBuf {
    services_root().join("content/config.yaml")
}

#[test]
fn assets_show_reports_details_for_an_existing_file() {
    let dir = assets_dir();
    std::fs::write(dir.join("logo.svg"), "<svg></svg>").unwrap();

    run(&["assets", "show", "logo.svg"]).unwrap();
    run(&["assets", "list"]).unwrap();
}

#[test]
fn assets_show_rejects_a_missing_asset() {
    assets_dir();

    let err = run(&["assets", "show", "absent.png"]).unwrap_err();
    assert!(format!("{err:#}").contains("'absent.png' not found"));
}

#[test]
fn assets_show_rejects_a_directory() {
    let dir = assets_dir();
    std::fs::create_dir_all(dir.join("icons")).unwrap();

    let err = run(&["assets", "show", "icons"]).unwrap_err();
    assert!(format!("{err:#}").contains("is not a file"));
}

#[test]
fn validate_fails_on_a_broken_content_config() {
    std::fs::write(
        content_config_path(),
        "content_sources: [this is not a map\n",
    )
    .unwrap();

    let err = run(&["validate"]).unwrap_err();
    assert!(format!("{err:#}").contains("web configuration is invalid"));
}

#[test]
fn validate_runs_each_category_filter() {
    std::fs::write(
        content_config_path(),
        "content_sources:\n  blog:\n    path: content/blog\n    source_id: blog\n    category_id: \
         blog\n    enabled: true\n",
    )
    .unwrap();

    for only in ["config", "templates", "assets"] {
        let result = run(&["validate", "--only", only]);
        if let Err(e) = result {
            assert!(
                format!("{e:#}").contains("web configuration is invalid"),
                "{e:#}"
            );
        }
    }
}

fn templates_dir() -> PathBuf {
    let dir = services_root().join("web/templates");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("templates.yaml"),
        "templates:\n  base:\n    content_types: [blog]\n",
    )
    .unwrap();
    std::fs::write(dir.join("base.html"), "<html></html>").unwrap();
    dir
}

#[test]
fn templates_subcommands_resolve_their_directory_from_the_profile() {
    let dir = templates_dir();

    run(&["templates", "list"]).unwrap();
    run(&["templates", "list", "--missing"]).unwrap();
    run(&["templates", "show", "base"]).unwrap();

    run(&[
        "templates",
        "create",
        "--name",
        "landing",
        "--content-types",
        "blog",
    ])
    .unwrap();
    let config = std::fs::read_to_string(dir.join("templates.yaml")).unwrap();
    assert!(config.contains("landing"), "{config}");

    run(&["templates", "edit", "landing", "--content-types", "docs"]).unwrap();
    let config = std::fs::read_to_string(dir.join("templates.yaml")).unwrap();
    assert!(config.contains("docs"), "{config}");

    run(&["templates", "delete", "landing", "--yes"]).unwrap();
    let config = std::fs::read_to_string(dir.join("templates.yaml")).unwrap();
    assert!(!config.contains("landing"), "{config}");
}

#[test]
fn templates_show_rejects_an_unknown_template() {
    templates_dir();

    let err = run(&["templates", "show", "ghost"]).unwrap_err();
    assert!(format!("{err:#}").contains("ghost"));
}

#[test]
fn sitemap_show_reads_the_profile_content_config() {
    std::fs::write(
        content_config_path(),
        "content_sources:\n  blog:\n    path: content/blog\n    source_id: blog\n    category_id: \
         blog\n    enabled: true\n    sitemap:\n      enabled: true\n      url_pattern: \
         /blog/{slug}\n      priority: 0.7\n      changefreq: weekly\n",
    )
    .unwrap();

    run(&["sitemap", "show"]).unwrap();
}
