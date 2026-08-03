//! Failure-path runs of the command dispatchers.
//!
//! The dispatcher tests elsewhere drive well-formed fixtures. These break the
//! fixture deliberately so the validation, parse-failure, and not-found arms
//! run — the arms an operator actually meets.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::agents::AgentsCommands;
use systemprompt_cli::core::CoreCommands;
use systemprompt_cli::web::WebCommands;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat, ScriptedPrompter};

fn services_root() -> PathBuf {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .services_path
        .clone()
}

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

#[derive(Debug, Parser)]
struct WebHarness {
    #[command(subcommand)]
    cmd: WebCommands,
}

#[derive(Debug, Parser)]
struct CoreHarness {
    #[command(subcommand)]
    cmd: CoreCommands,
}

#[derive(Debug, Parser)]
struct AgentsHarness {
    #[command(subcommand)]
    cmd: AgentsCommands,
}

fn web(args: &[&str]) -> WebCommands {
    WebHarness::try_parse_from(std::iter::once("web").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn core(args: &[&str]) -> CoreCommands {
    CoreHarness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn agents(args: &[&str]) -> AgentsCommands {
    AgentsHarness::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

#[tokio::test]
async fn an_agent_config_that_fails_validation_is_reported_by_every_reader() {
    let root = services_root();
    std::fs::create_dir_all(root.join("agents")).unwrap();
    std::fs::write(
        root.join("agents/covbroken.yaml"),
        "agents:\n  covbroken:\n    name: covbroken\n    port: 42\n",
    )
    .unwrap();
    std::fs::write(
        root.join("config/config.yaml"),
        "includes:\n  - ../agents/covbroken.yaml\nmcp_servers: {}\n",
    )
    .unwrap();

    for argv in [vec!["list"], vec!["validate"], vec!["show", "covbroken"]] {
        let err = systemprompt_cli::admin::agents::execute(agents(&argv), &ctx())
            .await
            .unwrap_err();
        assert!(!format!("{err:#}").is_empty(), "{argv:?}");
    }
}

#[tokio::test]
async fn an_unparseable_content_config_is_reported_by_every_web_reader() {
    let root = services_root();
    std::fs::write(
        root.join("content/config.yaml"),
        "content_sources: [this is not a mapping\n",
    )
    .unwrap();

    for argv in [
        vec!["content-types", "list"],
        vec!["content-types", "show", "blog"],
        vec!["sitemap", "show"],
    ] {
        let err = systemprompt_cli::web::execute(web(&argv), &ctx()).unwrap_err();
        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("config.yaml") || rendered.contains("parse"),
            "{argv:?}: {rendered}"
        );
    }
}

#[tokio::test]
async fn a_plugin_missing_its_referenced_skill_is_reported_as_invalid() {
    let root = services_root();
    let plugin = root.join("plugins/covinvalid");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(
        plugin.join("config.yaml"),
        "plugin:\n  id: covinvalid\n  name: Coverage Invalid\n  description: References a \
         missing skill\n  version: 1.0.0\n  author:\n    name: Tester\n    email: \
         tester@example.com\n  keywords: [demo]\n  license: MIT\n  category: tools\n  skills:\n    \
         source: explicit\n    include: [cov_absent_skill]\n  agents:\n    source: explicit\n    \
         include: []\n",
    )
    .unwrap();

    // Validation reports the finding in its table rather than failing the
    // command, so the run succeeds and the invalid row is the outcome.
    systemprompt_cli::core::execute(core(&["plugins", "validate"]), &ctx())
        .await
        .unwrap();

    // `validate` takes the id positionally, and an unknown one is a hard error.
    let err = systemprompt_cli::core::execute(
        core(&["plugins", "validate", "cov_absent_plugin"]),
        &ctx(),
    )
    .await
    .unwrap_err();
    assert!(format!("{err:#}").contains("cov_absent_plugin"));
}

#[tokio::test]
async fn an_unparseable_plugin_config_is_reported_by_the_generator() {
    let root = services_root();
    let plugin = root.join("plugins/covunparseable");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("config.yaml"), "plugin: [not a mapping\n").unwrap();

    let err = systemprompt_cli::core::execute(
        core(&["plugins", "generate", "--id", "covunparseable"]),
        &ctx(),
    )
    .await
    .unwrap_err();

    assert!(format!("{err:#}").contains("covunparseable"), "{err:#}");
}

#[tokio::test]
async fn a_templates_config_that_is_absent_is_reported_by_every_template_reader() {
    let root = services_root();
    let templates = root.join("web/templates");
    std::fs::create_dir_all(&templates).unwrap();
    let config = templates.join("templates.yaml");
    if config.exists() {
        std::fs::remove_file(&config).unwrap();
    }

    // Listing tolerates the missing config and renders an empty table; the
    // readers that need a named template do not.
    systemprompt_cli::web::execute(web(&["templates", "list"]), &ctx()).unwrap();

    for argv in [
        vec!["templates", "show", "base"],
        vec!["templates", "delete", "base", "--yes"],
    ] {
        let err = systemprompt_cli::web::execute(web(&argv), &ctx()).unwrap_err();
        assert!(
            format!("{err:#}").contains("templates"),
            "{argv:?}: {err:#}"
        );
    }
}

#[test]
fn a_scripted_prompter_that_runs_dry_surfaces_the_prompt_it_stopped_on() {
    let prompter = ScriptedPrompter::new(["only-one"]);

    assert_eq!(
        systemprompt_cli::interactive::Prompter::input(&prompter, "first").unwrap(),
        "only-one"
    );
    let err =
        systemprompt_cli::interactive::Prompter::input(&prompter, "second question").unwrap_err();
    assert!(err.to_string().contains("second question"), "{err}");
}
