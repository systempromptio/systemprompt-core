//! `admin issues list|show` against a migrated database through a real
//! `AppContext`, plus argument parsing for the triage states.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::admin::issues::{self, IssuesCommands};
use systemprompt_cli::descriptor::DataImpact;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_files::issues::IssueFiling;
use systemprompt_identifiers::{IssueReportId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::issues::{IssueContext, IssueKind, IssueSeverity, IssueStatus};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, test_app_context, test_database_url, test_db_pool,
};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: IssuesCommands,
}

fn parse(args: &[&str]) -> Result<IssuesCommands, clap::Error> {
    Harness::try_parse_from(std::iter::once("issues").chain(args.iter().copied())).map(|h| h.cmd)
}

async fn app() -> Arc<AppContext> {
    ensure_test_bootstrap();
    let url = test_database_url();
    let pool = test_db_pool().await;
    test_app_context(&pool, &url)
}

fn ctx(app: &Arc<AppContext>, format: OutputFormat) -> CommandContext {
    let cli = CliConfig::new()
        .with_interactive(false)
        .with_output_format(format);
    crate::boot_ctx::with_app_boot(cli, EnvOverrides::default(), Arc::clone(app))
}

async fn seed(app: &Arc<AppContext>, context: IssueContext) -> (UserId, IssueReportId) {
    let reporter = UserId::new(format!("cli-issues-{}", uuid::Uuid::new_v4()));
    let (id, filed) = app
        .documents()
        .issues()
        .file(IssueFiling {
            reporter: reporter.clone(),
            via_agent: None,
            kind: IssueKind::Bug,
            severity: IssueSeverity::High,
            title: format!("cli issue {}", uuid::Uuid::new_v4()),
            body: "steps".to_owned(),
            fingerprint_key: None,
            context,
            client: serde_json::Value::Null,
        })
        .await
        .unwrap();
    assert!(filed.inserted);
    (reporter, id)
}

#[test]
fn every_subcommand_preserves_data() {
    for args in [
        vec!["list"],
        vec!["show", "x"],
        vec!["triage", "x", "--status", "resolved"],
    ] {
        assert_eq!(parse(&args).unwrap().data_impact(), DataImpact::Preserving);
    }
}

#[test]
fn list_parses_filters_and_rejects_an_unknown_status() {
    let IssuesCommands::List(args) = parse(&[
        "list",
        "--status",
        "needs_info",
        "--reporter",
        "u1",
        "-n",
        "5",
    ])
    .unwrap() else {
        panic!("expected list");
    };
    assert_eq!(args.status, Some(IssueStatus::NeedsInfo));
    assert_eq!(args.reporter.as_deref(), Some("u1"));
    assert_eq!(args.limit, 5);
    assert!(parse(&["list", "--status", "bogus"]).is_err());
}

#[test]
fn triage_requires_a_status_and_carries_the_note() {
    assert!(parse(&["triage", "x"]).is_err());
    let IssuesCommands::Triage(args) =
        parse(&["triage", "abc", "--status", "closed", "--note", "dup"]).unwrap()
    else {
        panic!("expected triage");
    };
    assert_eq!(args.id, "abc");
    assert_eq!(args.status, IssueStatus::Closed);
    assert_eq!(args.note.as_deref(), Some("dup"));
}

#[tokio::test]
async fn list_filtered_by_reporter_and_status_renders_in_every_format() {
    let app = app().await;
    let (reporter, _) = seed(&app, IssueContext::default()).await;
    for format in [OutputFormat::Json, OutputFormat::Table, OutputFormat::Yaml] {
        issues::execute(
            parse(&["list", "--reporter", reporter.as_str(), "--status", "open"]).unwrap(),
            &ctx(&app, format),
        )
        .await
        .unwrap();
    }
    issues::execute(parse(&["list"]).unwrap(), &ctx(&app, OutputFormat::Json))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_renders_a_report_whose_context_names_platform_records() {
    let app = app().await;
    let (_, id) = seed(
        &app,
        IssueContext {
            trace: Some(TraceId::generate()),
            task: Some(TaskId::generate()),
            session: Some(SessionId::generate()),
            ..IssueContext::default()
        },
    )
    .await;
    issues::execute(
        parse(&["show", id.as_str()]).unwrap(),
        &ctx(&app, OutputFormat::Json),
    )
    .await
    .unwrap();

    let (_, bare) = seed(&app, IssueContext::default()).await;
    issues::execute(
        parse(&["show", bare.as_str()]).unwrap(),
        &ctx(&app, OutputFormat::Table),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn show_refuses_an_unknown_report_by_id() {
    let app = app().await;
    let missing = uuid::Uuid::new_v4().to_string();
    let err = issues::execute(
        parse(&["show", &missing]).unwrap(),
        &ctx(&app, OutputFormat::Json),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("Issue report not found: {missing}")
    );
}
