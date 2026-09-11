//! The interactive identifier prompt for `core content edit`: the candidate
//! list is fetched on the async path and the scripted selection resolves to
//! the chosen row. This path used to call `Handle::block_on` from inside the
//! runtime, which panics, so a passing run here is the regression check.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::core::content::edit;
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_content::models::CreateContentParams;
use systemprompt_content::{Content, ContentRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

async fn seed(pool: &DbPool, source: &str) -> Content {
    let slug = format!("promptslug{}", uuid::Uuid::new_v4().simple());
    let repo = ContentRepository::new(pool).unwrap();
    let params = CreateContentParams::new(
        slug.clone(),
        format!("Title for {slug}"),
        "A description".to_owned(),
        "The body".to_owned(),
        SourceId::new(source.to_owned()),
    )
    .with_version_hash("hash-1".to_owned());
    repo.create(&params).await.unwrap()
}

fn args(source: &str, set_values: Vec<String>) -> edit::EditArgs {
    edit::EditArgs {
        identifier: None,
        source: Some(source.to_owned()),
        set_values,
        public: false,
        private: false,
        body: None,
        body_file: None,
    }
}

#[tokio::test]
async fn interactive_edit_prompts_for_the_content_and_applies_the_change() {
    let pool = pool().await;
    let source = format!("promptsrc{}", uuid::Uuid::new_v4().simple());
    let seeded = seed(&pool, &source).await;
    let prompter = ScriptedPrompter::new(vec!["0"]);

    let out = edit::execute_with_pool(
        args(&source, vec!["title=Renamed by prompt".to_owned()]),
        &prompter,
        &pool,
        &CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
    )
    .await
    .expect("the interactive path completes inside the runtime");

    let rendered = serde_json::to_string(out.artifact()).unwrap();
    assert!(rendered.contains(seeded.id.as_str()), "{rendered}");
    let repo = ContentRepository::new(&pool).unwrap();
    let stored = repo.get_by_id(&seeded.id).await.unwrap().unwrap();
    assert_eq!(stored.title, "Renamed by prompt");
}

#[tokio::test]
async fn interactive_edit_with_no_candidates_reports_no_content() {
    let pool = pool().await;
    let source = format!("emptysrc{}", uuid::Uuid::new_v4().simple());
    let prompter = ScriptedPrompter::new(vec!["0"]);

    let err = edit::execute_with_pool(
        args(&source, vec![]),
        &prompter,
        &pool,
        &CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("No content found"), "{err}");
}
