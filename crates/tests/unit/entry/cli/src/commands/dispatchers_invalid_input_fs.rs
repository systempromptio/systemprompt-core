//! Failure-path runs of the command dispatchers.
//!
//! The dispatcher tests elsewhere drive well-formed fixtures. These break the
//! fixture deliberately so the validation, parse-failure, and not-found arms
//! run — the arms an operator actually meets.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use clap::Parser;
use systemprompt_cli::admin::agents::AgentsCommands;
use systemprompt_cli::web::WebCommands;
use systemprompt_cli::{CommandContext, EnvOverrides, ScriptedPrompter};
use systemprompt_extension::cli::{CliConfig, OutputFormat};

fn services_root() -> PathBuf {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .services_path
        .clone()
}

fn ctx() -> CommandContext {
    crate::boot_ctx::new_with_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

fn ctx_on(boot: &systemprompt_test_fixtures::TestBootstrap) -> CommandContext {
    crate::boot_ctx::new_on(
        boot,
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
struct AgentsHarness {
    #[command(subcommand)]
    cmd: AgentsCommands,
}

fn web(args: &[&str]) -> WebCommands {
    WebHarness::try_parse_from(std::iter::once("web").chain(args.iter().copied()))
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
    let boot = systemprompt_test_fixtures::init_unloadable_services_bootstrap(
        "http://127.0.0.1",
        "agents:\n  covbroken:\n    name: covbroken\n    port: 42\nmcp_servers: {}\n",
    );

    for argv in [vec!["list"], vec!["validate"], vec!["show", "covbroken"]] {
        let err = systemprompt_cli::admin::agents::execute(agents(&argv), &ctx_on(&boot))
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
        systemprompt_extension::cli::Prompter::input(&prompter, "first").unwrap(),
        "only-one"
    );
    let err =
        systemprompt_extension::cli::Prompter::input(&prompter, "second question").unwrap_err();
    assert!(err.to_string().contains("second question"), "{err}");
}
