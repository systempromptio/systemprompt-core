//! The `admin evals` command tree.
//!
//! Reviewed case capture intentionally constructs no AI provider. Paid
//! evaluation is available only through experiments.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::OnceLock;

use clap::Parser;
use systemprompt_cli::admin::evals::{EvalsCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context, fixture_db_pool, init_services_bootstrap,
};

const GATEWAY_YAML: &str = r#"
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: http://127.0.0.1:1
    api_key_secret: anthropic_api_key
    models:
      - id: claude-fixture-1
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
          cache_read_per_million: 0.0
gateway:
  enabled: true
  allow_unlisted_models: false
  routes:
    - id: claude
      model_pattern: "claude-*"
      provider: anthropic
"#;

const SERVICES_YAML: &str = r#"ai:
  default_provider: anthropic
  providers:
    anthropic:
      enabled: true
      default_model: claude-fixture-1
"#;

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn boot() -> &'static TestBootstrap {
    BOOT.get_or_init(|| init_services_bootstrap(&format!("{SERVICES_YAML}{GATEWAY_YAML}")))
}

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: EvalsCommands,
}

fn parse(args: &[&str]) -> EvalsCommands {
    Harness::try_parse_from(std::iter::once("evals").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}

pub(super) async fn pool() -> DbPool {
    let b = boot();
    fixture_db_pool(&b.database_url)
        .await
        .expect("the evals command tests need a reachable test database")
}

pub(super) fn ctx(pool: &DbPool) -> CommandContext {
    let b = boot();
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(pool, &b.database_url).expect("app context"),
    )
}

async fn run(args: &[&str]) -> anyhow::Result<()> {
    let pool = pool().await;
    execute(parse(args), &ctx(&pool)).await
}

fn message(err: &anyhow::Error) -> String {
    format!("{err:#}")
}

// Why: the assertions below name the specific refusal rather than accepting
// any error. An earlier draft asserted only that *some* error came back, and
// passed while every command was failing on `AiService::new` — reporting
// success for refusals it never reached.
#[test]
fn the_judge_run_commands_are_gone() {
    for removed in ["run", "replay", "list", "show"] {
        assert!(
            Harness::try_parse_from(["evals", removed]).is_err(),
            "`admin evals {removed}` must not parse: paid evaluation runs only through experiments"
        );
    }
}

#[tokio::test]
async fn the_eval_context_builds_without_an_inference_provider() {
    let err = run(&["promote", "eval-request-that-does-not-exist"])
        .await
        .expect_err("promote reaches its own refusal, so the context was built");
    assert!(
        message(&err).contains("eval-request-that-does-not-exist"),
        "the refusal names the request rather than failing earlier in the chain: {}",
        message(&err)
    );
}

#[tokio::test]
async fn promoting_a_result_that_does_not_exist_names_the_result() {
    let err = run(&["promote", "eval-result-that-does-not-exist"])
        .await
        .expect_err("promoting an unknown result must not create a case");

    assert!(
        message(&err).contains("eval-result-that-does-not-exist"),
        "the refusal should name the result: {}",
        message(&err)
    );
}
