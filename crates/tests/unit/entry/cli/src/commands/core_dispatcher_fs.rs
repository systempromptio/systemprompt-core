//! Tests that drive the profile-backed arms of the `core` command group
//! through `core::execute`.
//!
//! The skills, hooks, and plugins arms read the profile's services tree, so
//! the bootstrap fixture's tempdir is populated per test and each dispatcher
//! arm is entered with real content behind it.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};

const PLUGIN_YAML: &str = r#"plugin:
  id: covplugin
  name: Coverage Plugin
  description: Fixture plugin
  version: 1.0.0
  author:
    name: Tester
    email: tester@example.com
  keywords: [demo]
  license: MIT
  category: tools
  skills:
    source: explicit
    include: [covskill]
  agents:
    source: explicit
    include: []
"#;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: CoreCommands,
}

fn parse(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
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

fn services_root() -> PathBuf {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    boot.services_path.clone()
}

fn seed_skill() -> PathBuf {
    let root = services_root();
    let skill = root.join("skills/covskill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: covskill\nname: Coverage Skill\ndescription: A fixture skill\n",
    )
    .unwrap();
    std::fs::write(skill.join("index.md"), "# Coverage Skill\n\nBody.\n").unwrap();
    root
}

fn seed_plugin() -> PathBuf {
    let root = seed_skill();
    let plugin = root.join("plugins/covplugin");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("config.yaml"), PLUGIN_YAML).unwrap();
    root
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    core::execute(parse(args), &ctx()).await
}

#[tokio::test]
async fn skills_arms_list_and_show_a_seeded_skill() {
    seed_skill();

    run(&["skills", "list"]).await.unwrap();
    run(&["skills", "show", "covskill"]).await.unwrap();
}

#[tokio::test]
async fn skills_show_rejects_an_unknown_skill() {
    seed_skill();

    let err = run(&["skills", "show", "cov_no_such_skill"])
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("cov_no_such_skill"));
}

#[tokio::test]
async fn plugins_arms_list_show_and_validate_a_seeded_plugin() {
    seed_plugin();

    run(&["plugins", "list"]).await.unwrap();
    run(&["plugins", "show", "covplugin"]).await.unwrap();
    run(&["plugins", "validate"]).await.unwrap();
}

#[tokio::test]
async fn plugins_generate_materialises_output_for_a_seeded_plugin() {
    let root = seed_plugin();
    let out = tempfile::tempdir().unwrap();

    run(&[
        "plugins",
        "generate",
        "--id",
        "covplugin",
        "--output-dir",
        out.path().to_str().unwrap(),
    ])
    .await
    .unwrap();

    assert!(out.path().join(".claude-plugin/plugin.json").exists());
    assert!(root.join("plugins/covplugin/config.yaml").exists());
}

#[tokio::test]
async fn plugins_generate_rejects_an_unknown_id() {
    seed_plugin();

    let err = run(&["plugins", "generate", "--id", "cov_absent_plugin"])
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("cov_absent_plugin"));
}

#[tokio::test]
async fn hooks_arms_list_and_validate_without_hook_definitions() {
    seed_plugin();

    run(&["hooks", "list"]).await.unwrap();
    run(&["hooks", "validate"]).await.unwrap();
}

#[tokio::test]
async fn plugins_generate_materialises_enabled_and_default_marketplace_catalogs() {
    let root = seed_plugin();
    std::fs::write(
        root.join("config/config.yaml"),
        r#"
settings:
  default_marketplace_id: primary
marketplaces:
  primary:
    id: primary
    name: Primary
    description: Primary plugin catalog
    version: 2.0.0
    enabled: true
    author: { name: Catalog Owner, email: owner@example.com }
    license: MIT
    plugins:
      source: explicit
      include: [covplugin]
  disabled:
    id: disabled
    name: Disabled
    description: Disabled plugin catalog
    version: 1.0.0
    enabled: false
    author: { name: Catalog Owner, email: owner@example.com }
    license: MIT
    plugins:
      source: explicit
      include: [covplugin]
"#,
    )
    .unwrap();
    systemprompt_test_fixtures::refresh_services_config();
    let out = tempfile::tempdir().unwrap();

    run(&[
        "plugins",
        "generate",
        "--id",
        "covplugin",
        "--output-dir",
        out.path().to_str().unwrap(),
    ])
    .await
    .expect("generate plugin and configured marketplace catalogs");

    let catalog_dir = root
        .parent()
        .unwrap()
        .join("storage/files/plugins/.claude-plugin");
    let named = catalog_dir.join("marketplace-primary.json");
    let default = catalog_dir.join("marketplace.json");
    assert!(named.exists(), "enabled named catalog must be generated");
    assert_eq!(
        std::fs::read(&named).unwrap(),
        std::fs::read(&default).unwrap(),
        "the configured default catalog must also own marketplace.json"
    );
    assert!(
        !catalog_dir.join("marketplace-disabled.json").exists(),
        "a disabled catalog must not be published"
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(named).unwrap()).unwrap();
    assert_eq!(manifest["name"], "primary", "{manifest}");
    assert_eq!(manifest["owner"]["name"], "Catalog Owner", "{manifest}");
    assert_eq!(manifest["metadata"]["version"], "2.0.0", "{manifest}");
    let plugins = manifest["plugins"].as_array().expect("catalog plugins");
    assert_eq!(plugins.len(), 1, "{manifest}");
    assert_eq!(plugins[0]["name"], "covplugin", "{manifest}");
}

#[tokio::test]
async fn plugins_generate_preserves_an_existing_catalog_path_when_marketplace_publish_fails() {
    let root = seed_plugin();
    std::fs::write(
        root.join("config/config.yaml"),
        r#"
marketplaces:
  primary:
    id: primary
    name: Primary
    description: Primary plugin catalog
    version: 2.0.0
    enabled: true
    author: { name: Catalog Owner, email: owner@example.com }
    license: MIT
    plugins:
      source: explicit
      include: [covplugin]
"#,
    )
    .unwrap();
    systemprompt_test_fixtures::refresh_services_config();
    let output = tempfile::tempdir().unwrap();
    let catalog_parent = root.parent().unwrap().join("storage/files/plugins");
    std::fs::create_dir_all(&catalog_parent).unwrap();
    let blocked = catalog_parent.join(".claude-plugin");
    std::fs::write(&blocked, "operator-owned sentinel\n").unwrap();

    let error = run(&[
        "plugins",
        "generate",
        "--id",
        "covplugin",
        "--output-dir",
        output.path().to_str().unwrap(),
    ])
    .await
    .expect_err("a file at the catalog directory must prevent marketplace publication");
    assert!(
        matches!(error.downcast_ref::<std::io::Error>(), Some(io) if io.kind() == std::io::ErrorKind::AlreadyExists),
        "{error:#}"
    );
    assert_eq!(
        std::fs::read_to_string(&blocked).unwrap(),
        "operator-owned sentinel\n",
        "failed publication must not replace an operator-owned path"
    );
    assert!(
        output.path().join(".claude-plugin/plugin.json").exists(),
        "the requested plugin artifact is complete before catalog publication is attempted"
    );
}
