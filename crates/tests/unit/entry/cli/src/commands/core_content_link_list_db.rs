//! `core content link list` — the scoping on a link listing.
//!
//! The command refuses to run without a filter, which is the interesting part:
//! campaign links carry click counts and target URLs, so an unfiltered listing
//! would hand every caller the whole table.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::content::link::list::{ListArgs, execute};
use systemprompt_cli::core::content::{self, ContentCommands};
use systemprompt_cli::shared::CommandOutput;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct ContentHarness {
    #[command(subcommand)]
    cmd: ContentCommands,
}

fn parse(args: &[&str]) -> ContentCommands {
    ContentHarness::try_parse_from(std::iter::once("content").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().expect("DATABASE_URL"))
        .await
        .expect("the link list tests need a reachable test database")
}

fn ctx(pool: &DbPool) -> CommandContext {
    let url = fixture_database_url().expect("DATABASE_URL");
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(pool, &url).expect("app context"),
    )
}

/// `campaign_links.source_content_id` is a real foreign key, so a link bound
/// to content needs a content row to point at.
async fn seed_content(pool: &DbPool) -> String {
    let repo = ContentRepository::new(pool).expect("content repository");
    let slug = format!("ll-{}", Uuid::new_v4().simple());
    let params = CreateContentParams::new(
        slug.clone(),
        format!("Title {slug}"),
        "desc".to_owned(),
        "body".to_owned(),
        SourceId::new(format!("src-{}", Uuid::new_v4().simple())),
    )
    .with_version_hash("h1".to_owned());
    repo.create(&params)
        .await
        .expect("seed content")
        .id
        .as_str()
        .to_owned()
}

/// Generate a link, optionally bound to a campaign and a source content id.
async fn seed_link(
    ctx: &CommandContext,
    campaign: Option<&str>,
    content_id: Option<&str>,
) -> String {
    let target = format!("https://example.com/ll-{}", Uuid::new_v4().simple());
    let mut args = vec![
        "link",
        "generate",
        "--url",
        &target,
        "--link-type",
        "redirect",
    ];
    if let Some(c) = campaign {
        args.push("--campaign");
        args.push(c);
    }
    if let Some(c) = content_id {
        args.push("--content");
        args.push(c);
    }
    content::execute(parse(&args), ctx)
        .await
        .expect("generate link");
    target
}

fn targets(output: &CommandOutput) -> Vec<String> {
    let artifact = serde_json::to_value(output.artifact()).expect("serialise artifact");
    artifact["items"]
        .as_array()
        .unwrap_or_else(|| panic!("no items in artifact: {artifact}"))
        .iter()
        .filter_map(|row| row["target_url"].as_str().map(str::to_owned))
        .collect()
}

async fn list(
    ctx: &CommandContext,
    campaign: Option<&str>,
    content_id: Option<&str>,
) -> CommandOutput {
    execute(
        ListArgs {
            campaign: campaign.map(str::to_owned),
            content: content_id.map(str::to_owned),
        },
        ctx,
    )
    .await
    .expect("listing should succeed")
}

// Why: with neither filter the query would have no scope at all. Campaign
// links carry target URLs and click counts, so an unfiltered listing hands
// back the whole table rather than the caller's own links.
#[tokio::test]
async fn listing_without_a_filter_is_refused_rather_than_listing_everything() {
    let pool = pool().await;

    let err = execute(
        ListArgs {
            campaign: None,
            content: None,
        },
        &ctx(&pool),
    )
    .await
    .expect_err("an unscoped listing must not be served");

    assert!(
        format!("{err:#}").contains("--campaign"),
        "the refusal should name the filters available: {err:#}"
    );
}

#[tokio::test]
async fn a_campaign_filter_returns_only_that_campaigns_links() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let mine = format!("camp-{}", Uuid::new_v4().simple());
    let theirs = format!("camp-{}", Uuid::new_v4().simple());

    let a = seed_link(&ctx, Some(&mine), None).await;
    let b = seed_link(&ctx, Some(&mine), None).await;
    let other = seed_link(&ctx, Some(&theirs), None).await;

    let listed = targets(&list(&ctx, Some(&mine), None).await);

    assert!(listed.contains(&a) && listed.contains(&b));
    assert!(
        !listed.contains(&other),
        "another campaign's link must not appear: {listed:?}"
    );
    assert_eq!(listed.len(), 2, "exactly the campaign's own links");
}

#[tokio::test]
async fn a_content_filter_returns_only_links_from_that_source() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let mine = seed_content(&pool).await;
    let theirs = seed_content(&pool).await;

    let a = seed_link(&ctx, None, Some(&mine)).await;
    let other = seed_link(&ctx, None, Some(&theirs)).await;

    let listed = targets(&list(&ctx, None, Some(&mine)).await);

    assert_eq!(listed, vec![a], "only this source's links belong here");
    assert!(!listed.contains(&other));
}

// Why: campaign takes precedence when both are given. Silently intersecting or
// switching to the content filter would return a set the caller did not ask
// for, with nothing in the output saying which filter was applied.
#[tokio::test]
async fn campaign_wins_when_both_filters_are_supplied() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let campaign = format!("camp-{}", Uuid::new_v4().simple());
    let content_id = seed_content(&pool).await;

    let by_campaign = seed_link(&ctx, Some(&campaign), None).await;
    let by_content = seed_link(&ctx, None, Some(&content_id)).await;

    let listed = targets(&list(&ctx, Some(&campaign), Some(&content_id)).await);

    assert_eq!(
        listed,
        vec![by_campaign],
        "the campaign filter is checked first and wins"
    );
    assert!(!listed.contains(&by_content));
}

#[tokio::test]
async fn a_campaign_with_no_links_lists_nothing_rather_than_failing() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let empty = format!("camp-{}", Uuid::new_v4().simple());

    assert!(targets(&list(&ctx, Some(&empty), None).await).is_empty());
}

// Why: a link that has never been clicked has a NULL count. Rendering it as
// absent, or failing, would make an unclicked link indistinguishable from a
// missing one in a performance report.
#[tokio::test]
async fn a_link_that_was_never_clicked_reports_zero_clicks() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let campaign = format!("camp-{}", Uuid::new_v4().simple());
    seed_link(&ctx, Some(&campaign), None).await;

    let output = list(&ctx, Some(&campaign), None).await;
    let artifact = serde_json::to_value(output.artifact()).expect("serialise");
    let row = &artifact["items"].as_array().expect("items")[0];

    assert_eq!(
        row["click_count"].as_i64(),
        Some(0),
        "an unclicked link reports zero, not null: {row}"
    );
}
