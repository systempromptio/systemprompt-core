//! DB-backed tests for the pool-seamed `core content files` command tree
//! (list/link/unlink/featured), driving `execute_with_pool` against a fixture
//! pool with real content and file rows.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::Value;
use systemprompt_cli::CliConfig;
use systemprompt_cli::core::content::files::{featured, link, list, unlink};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::shared::CommandOutput;
use systemprompt_content::ContentRepository;
use systemprompt_content::models::CreateContentParams;
use systemprompt_database::DbPool;
use systemprompt_files::{FileRepository, FileRole};
use systemprompt_identifiers::{ContentId, FileId, SourceId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn artifact_json(out: &CommandOutput) -> Value {
    serde_json::to_value(out.artifact()).unwrap()
}

fn contains(out: &CommandOutput, needle: &str) -> bool {
    serde_json::to_string(&artifact_json(out))
        .unwrap()
        .contains(needle)
}

async fn seed_content(pool: &DbPool) -> ContentId {
    let repo = ContentRepository::new(pool).unwrap();
    let slug = format!("cf-{}", Uuid::new_v4().simple());
    let params = CreateContentParams::new(
        slug.clone(),
        format!("Title {slug}"),
        "desc".to_owned(),
        "body".to_owned(),
        SourceId::new(format!("src-{}", Uuid::new_v4().simple())),
    )
    .with_version_hash("h1".to_owned());
    repo.create(&params).await.unwrap().id
}

async fn seed_file(pool: &DbPool) -> String {
    let id = Uuid::new_v4();
    let path = format!("/uploads/cf/{id}.png");
    let url = format!("https://files.invalid/{id}");
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content) \
         VALUES ($1, $2, $3, 'image/png', 64, false)",
    )
    .bind(id)
    .bind(&path)
    .bind(&url)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    id.to_string()
}

fn link_args(file: &str, content: &ContentId, role: link::FileRoleArg) -> link::LinkArgs {
    link::LinkArgs {
        file: file.to_owned(),
        content: content.as_str().to_owned(),
        role,
        order: 0,
    }
}

async fn linked_roles(pool: &DbPool, file: &str) -> Vec<FileRole> {
    let repo = FileRepository::new(pool).unwrap();
    repo.list_content_by_file(&FileId::new(file.to_owned()))
        .await
        .unwrap()
        .into_iter()
        .map(|cf| cf.role)
        .collect()
}

#[tokio::test]
async fn link_attaches_file_to_content() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;

    let out = link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(contains(&out, &file));
    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn list_by_content_and_by_file_reflect_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Inline),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let by_content = list::execute_with_pool(
        list::ListArgs {
            content: Some(content.as_str().to_owned()),
            file: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert!(contains(&by_content, &file));

    let by_file = list::execute_with_pool(
        list::ListArgs {
            content: None,
            file: Some(file.clone()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert!(contains(&by_file, content.as_str()));
}

#[tokio::test]
async fn list_requires_exactly_one_filter() {
    let pool = pool().await;

    let none = list::execute_with_pool(
        list::ListArgs {
            content: None,
            file: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(none.to_string().contains("Either --content or --file"));

    let both = list::execute_with_pool(
        list::ListArgs {
            content: Some("c".to_owned()),
            file: Some("f".to_owned()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(both.to_string().contains("Cannot specify both"));
}

#[tokio::test]
async fn list_by_file_rejects_bad_uuid() {
    let pool = pool().await;
    let err = list::execute_with_pool(
        list::ListArgs {
            content: None,
            file: Some("nope".to_owned()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Invalid file ID format"));
}

fn unlink_args(file: &str, content: &ContentId, yes: bool, dry_run: bool) -> unlink::UnlinkArgs {
    unlink::UnlinkArgs {
        file: file.to_owned(),
        content: content.as_str().to_owned(),
        yes,
        dry_run,
    }
}

#[tokio::test]
async fn unlink_with_yes_removes_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    unlink::execute_with_pool(
        unlink_args(&file, &content, true, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(linked_roles(&pool, &file).await.is_empty());
}

#[tokio::test]
async fn unlink_dry_run_preserves_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    unlink::execute_with_pool(
        unlink_args(&file, &content, true, true),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn unlink_non_interactive_without_yes_errors() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let err = unlink::execute_with_pool(
        unlink_args(&file, &content, false, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("--yes is required"));
    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn unlink_interactive_confirm_no_preserves_link() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    let err = unlink::execute_with_pool(
        unlink_args(&file, &content, false, false),
        &ScriptedPrompter::new(vec!["n"]),
        &pool,
        &CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("cancelled"));
    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Attachment]);
}

#[tokio::test]
async fn featured_set_then_get_returns_image() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;
    link::execute_with_pool(
        link_args(&file, &content, link::FileRoleArg::Attachment),
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: Some(file.clone()),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert_eq!(linked_roles(&pool, &file).await, vec![FileRole::Featured]);

    let out = featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    assert!(contains(&out, &file));
}

#[tokio::test]
async fn featured_get_none_reports_absence() {
    let pool = pool().await;
    let content = seed_content(&pool).await;

    let out = featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: None,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(contains(&out, "No featured image set"));
}

#[tokio::test]
async fn featured_set_unlinked_file_errors() {
    let pool = pool().await;
    let content = seed_content(&pool).await;
    let file = seed_file(&pool).await;

    let err = featured::execute_with_pool(
        featured::FeaturedArgs {
            content: content.as_str().to_owned(),
            set: Some(file),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("not linked"));
}
