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
