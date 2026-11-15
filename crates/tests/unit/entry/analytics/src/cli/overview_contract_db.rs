//! The `analytics overview` wire contract downstream MCP servers deserialize:
//! one presentation card titled "Analytics Overview" with its sections.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_analytics_entry::cli::overview::{OverviewArgs, execute_with_pool};
use systemprompt_cli::CliConfig;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::test_db_pool;

#[tokio::test]
async fn overview_emits_its_presentation_card() {
    let pool = test_db_pool().await;
    let out = execute_with_pool(
        OverviewArgs {
            since: Some("24h".to_owned()),
            until: None,
            export: None,
        },
        &DatabaseContext::from_pool(pool),
        &CliConfig::new().with_interactive(false),
    )
    .await
    .expect("overview runs against the test database");

    let card = serde_json::to_value(out.artifact()).expect("serialize the artifact");
    assert_eq!(card["artifact_type"], "presentation_card", "{card}");
    assert_eq!(card["title"], "Analytics Overview", "{card}");
    assert!(card["sections"].as_array().is_some(), "{card}");
}
