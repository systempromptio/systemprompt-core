//! The `admin evals` command tree.
//!
//! Every subcommand opens with `eval_context`, a hard chain ending in
//! `AiService::new`. That needs both halves of the AI configuration — the
//! profile's `providers:` registry for connectivity and the services config's
//! `ai.providers` for the policy saying which are enabled — so the suite boots
//! with `init_ai_bootstrap` rather than the shared fixture, whose services
//! config has no `ai:` section at all.
//!
//! The provider endpoint is a closed port. The read commands never dial it,
//! and `run` does — completing anyway, because the loop swallows a judge
//! failure per sampled request.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::OnceLock;

use clap::Parser;
use systemprompt_cli::admin::evals::{EvalsCommands, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context, fixture_db_pool, init_ai_bootstrap,
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
    BOOT.get_or_init(|| init_ai_bootstrap(GATEWAY_YAML, SERVICES_YAML))
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

// Why: this is the suite's precondition. If any link of `eval_context` fails —
// app context, services config, profile, MCP tool provider, analytics session
// provider, `AiService::new` — every subcommand covers one line and stops.
#[tokio::test]
async fn the_eval_context_chain_builds_against_a_provider_profile() {
    run(&["list"])
        .await
        .expect("listing runs should build the whole eval context");
}

#[tokio::test]
async fn listing_accepts_an_explicit_limit() {
    run(&["list", "--limit", "5"]).await.expect("list --limit");
}

// Why: the assertions below name the specific refusal rather than accepting
// any error. An earlier draft asserted only that *some* error came back, and
// passed while every command was failing on `AiService::new` — reporting
// success for refusals it never reached.
#[tokio::test]
async fn showing_a_run_that_does_not_exist_names_the_run_it_could_not_find() {
    let err = run(&["show", "eval-run-that-does-not-exist"])
        .await
        .expect_err("an unknown run id must not render as an empty report");

    assert!(
        message(&err).contains("eval-run-that-does-not-exist"),
        "the error should name the run rather than fail earlier in the chain: {}",
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

#[tokio::test]
async fn replaying_a_run_with_no_failures_names_the_run() {
    let err = run(&["replay", "eval-run-that-does-not-exist"])
        .await
        .expect_err("replaying an unknown run must not start one");

    assert!(
        message(&err).contains("eval-run-that-does-not-exist"),
        "the refusal should name the run: {}",
        message(&err)
    );
}

// Why: an unreachable provider does NOT fail the command. `AutoImproveLoop`
// swallows a judge failure per sampled request so one bad response cannot
// discard a whole run, so the command completes and reports nothing scored.
// That is surprising enough to pin: if judge failures were ever made fatal,
// this test fails and forces the decision to be made deliberately rather than
// discovered by an operator whose run aborted halfway.
//
// The swallow itself is asserted where it can be observed, against the report
// counters, in the evaluation service tests.
#[tokio::test]
async fn a_run_against_an_unreachable_provider_completes_rather_than_aborting() {
    run(&["run", "--sample-size", "1"])
        .await
        .expect("a judge failure is swallowed per request, so the run completes");
}
