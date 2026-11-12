//! DB-backed tests for the pool-seamed `core files ai` (list/show) and
//! `core files delete` commands, driving `execute_with_pool` directly against
//! a fixture pool.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use serde_json::Value;
use systemprompt_cli::CliConfig;
use systemprompt_cli::core::files::{ai, delete};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::shared::CommandOutput;
use systemprompt_database::DbPool;
use systemprompt_files::FileRepository;
use systemprompt_identifiers::FileId;
use systemprompt_storage::{FileStorageBackend, build_file_storage};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{FileStorage, StoredFileId};
use uuid::Uuid;


fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

async fn seed_file(pool: &DbPool, ai_content: bool, user_id: &str) -> String {
    let id = Uuid::new_v4();
    let path = format!("/uploads/ai-del/{id}.png");
    let url = format!("https://files.invalid/{id}");
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content, user_id) \
         VALUES ($1, $2, $3, 'image/png', 128, $4, $5)",
    )
    .bind(id)
    .bind(&path)
    .bind(&url)
    .bind(ai_content)
    .bind(user_id)
    .execute(pool.pool().as_ref())
    .await
    .unwrap();
    id.to_string()
}

fn artifact_json(out: &CommandOutput) -> Value {
    serde_json::to_value(out.artifact()).unwrap()
}

fn contains_id(out: &CommandOutput, id: &str) -> bool {
    serde_json::to_string(&artifact_json(out))
        .unwrap()
        .contains(id)
}

fn list_args(limit: i64, user: Option<String>) -> ai::list::ListArgs {
    ai::list::ListArgs {
        limit,
        offset: 0,
        user,
    }
}

#[tokio::test]
async fn ai_list_includes_ai_files_and_excludes_regular() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let ai_id = seed_file(&pool, true, &user).await;
    let plain_id = seed_file(&pool, false, &user).await;

    let out = ai::list::execute_with_pool(list_args(500, None), &pool, &cfg())
        .await
        .unwrap();

    assert!(contains_id(&out, &ai_id), "ai file must appear in ai list");
    assert!(
        !contains_id(&out, &plain_id),
        "non-ai file must not appear in ai list"
    );
}

#[tokio::test]
async fn ai_list_filters_by_user() {
    let pool = test_db_pool().await;
    let mine = format!("mine-{}", Uuid::new_v4().simple());
    let other = format!("other-{}", Uuid::new_v4().simple());
    let my_id = seed_file(&pool, true, &mine).await;
    let other_id = seed_file(&pool, true, &other).await;

    let out = ai::list::execute_with_pool(list_args(500, Some(mine)), &pool, &cfg())
        .await
        .unwrap();

    assert!(contains_id(&out, &my_id));
    assert!(!contains_id(&out, &other_id));
}

#[tokio::test]
async fn ai_show_renders_ai_file() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let id = seed_file(&pool, true, &user).await;

    let out = ai::show::execute_with_pool(ai::show::ShowArgs { file: id.clone() }, &pool, &cfg())
        .await
        .unwrap();

    assert!(contains_id(&out, &id));
}

#[tokio::test]
async fn ai_show_rejects_non_ai_file() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let id = seed_file(&pool, false, &user).await;

    let err = ai::show::execute_with_pool(ai::show::ShowArgs { file: id }, &pool, &cfg())
        .await
        .unwrap_err();

    assert!(err.to_string().contains("not an AI-generated image"));
}

#[tokio::test]
async fn ai_show_rejects_bad_uuid() {
    let pool = test_db_pool().await;
    let err = ai::show::execute_with_pool(
        ai::show::ShowArgs {
            file: "not-a-uuid".to_owned(),
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("Invalid file ID format"));
}

#[tokio::test]
async fn ai_show_missing_file_errors() {
    let pool = test_db_pool().await;
    let ghost = Uuid::new_v4().to_string();
    let err = ai::show::execute_with_pool(ai::show::ShowArgs { file: ghost }, &pool, &cfg())
        .await
        .unwrap_err();

    assert!(err.to_string().contains("File not found"));
}

fn delete_args(file: &str, yes: bool, dry_run: bool) -> delete::DeleteArgs {
    delete::DeleteArgs {
        file: file.to_owned(),
        yes,
        dry_run,
    }
}

fn unused_storage() -> Arc<dyn FileStorage> {
    storage_at(&std::env::temp_dir().join(format!("files-del-{}", Uuid::new_v4().simple())))
}

fn storage_at(root: &std::path::Path) -> Arc<dyn FileStorage> {
    build_file_storage(FileStorageBackend::Local {
        root: root.to_path_buf(),
    })
}

async fn seed_stored_file(pool: &DbPool, storage: &Arc<dyn FileStorage>) -> (String, String) {
    let id = Uuid::new_v4();
    let stored = storage
        .store(
            std::path::Path::new(&format!("files/uploads/contexts/c/images/{id}.png")),
            b"\x89PNG",
        )
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, size_bytes) VALUES ($1, $2, $3, \
         'image/png', 4)",
    )
    .bind(id)
    .bind(stored.as_str())
    .bind(format!("/files/uploads/contexts/c/images/{id}.png"))
    .execute(pool.pool().as_ref())
    .await
    .unwrap();
    (id.to_string(), stored.as_str().to_owned())
}

async fn file_exists(pool: &DbPool, id: &str) -> bool {
    let repo = FileRepository::new(pool);
    repo.find_by_id(&FileId::new(id.to_owned()))
        .await
        .unwrap()
        .is_some()
}

#[tokio::test]
async fn delete_with_yes_removes_file() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let id = seed_file(&pool, false, &user).await;

    let out = delete::execute_with_pool(
        delete_args(&id, true, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        unused_storage(),
        &cfg(),
    )
    .await
    .unwrap();

    assert!(contains_id(&out, &id));
    assert!(!file_exists(&pool, &id).await);
}

#[tokio::test]
async fn delete_dry_run_preserves_file() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let id = seed_file(&pool, false, &user).await;

    delete::execute_with_pool(
        delete_args(&id, false, true),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        unused_storage(),
        &cfg(),
    )
    .await
    .unwrap();

    assert!(file_exists(&pool, &id).await);
}

#[tokio::test]
async fn delete_non_interactive_without_yes_errors() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let id = seed_file(&pool, false, &user).await;

    let err = delete::execute_with_pool(
        delete_args(&id, false, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        unused_storage(),
        &cfg(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("--yes is required"));
    assert!(file_exists(&pool, &id).await);
}

#[tokio::test]
async fn delete_interactive_confirm_yes_removes_file() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let id = seed_file(&pool, false, &user).await;

    delete::execute_with_pool(
        delete_args(&id, false, false),
        &ScriptedPrompter::new(vec!["y"]),
        &pool,
        unused_storage(),
        &CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
    )
    .await
    .unwrap();

    assert!(!file_exists(&pool, &id).await);
}

#[tokio::test]
async fn delete_interactive_confirm_no_preserves_file() {
    let pool = test_db_pool().await;
    let user = format!("u-{}", Uuid::new_v4().simple());
    let id = seed_file(&pool, false, &user).await;

    let err = delete::execute_with_pool(
        delete_args(&id, false, false),
        &ScriptedPrompter::new(vec!["n"]),
        &pool,
        unused_storage(),
        &CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("cancelled"));
    assert!(file_exists(&pool, &id).await);
}

#[tokio::test]
async fn delete_missing_file_errors() {
    let pool = test_db_pool().await;
    let ghost = Uuid::new_v4().to_string();
    let err = delete::execute_with_pool(
        delete_args(&ghost, true, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        unused_storage(),
        &cfg(),
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("File not found"));
}

#[tokio::test]
async fn delete_removes_the_stored_object_with_the_row() {
    let pool = test_db_pool().await;
    let dir = tempfile::tempdir().unwrap();
    let storage = storage_at(dir.path());
    let (id, stored) = seed_stored_file(&pool, &storage).await;
    assert!(dir.path().join(&stored).is_file());

    let out = delete::execute_with_pool(
        delete_args(&id, true, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        Arc::clone(&storage),
        &cfg(),
    )
    .await
    .unwrap();

    assert!(
        artifact_json(&out)
            .to_string()
            .contains("stored object deleted")
    );
    assert!(!file_exists(&pool, &id).await);
    assert!(!dir.path().join(&stored).exists());
    assert!(!storage.exists(&StoredFileId::new(stored)).await.unwrap());
}

#[tokio::test]
async fn delete_dry_run_preserves_the_stored_object() {
    let pool = test_db_pool().await;
    let dir = tempfile::tempdir().unwrap();
    let storage = storage_at(dir.path());
    let (id, stored) = seed_stored_file(&pool, &storage).await;

    delete::execute_with_pool(
        delete_args(&id, false, true),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        Arc::clone(&storage),
        &cfg(),
    )
    .await
    .unwrap();

    assert!(file_exists(&pool, &id).await);
    assert!(dir.path().join(&stored).is_file());
}

#[tokio::test]
async fn delete_of_a_row_whose_object_is_gone_removes_the_row() {
    let pool = test_db_pool().await;
    let dir = tempfile::tempdir().unwrap();
    let storage = storage_at(dir.path());
    let (id, stored) = seed_stored_file(&pool, &storage).await;
    std::fs::remove_file(dir.path().join(&stored)).unwrap();

    let out = delete::execute_with_pool(
        delete_args(&id, true, false),
        &ScriptedPrompter::new(Vec::<String>::new()),
        &pool,
        storage,
        &cfg(),
    )
    .await
    .unwrap();

    assert!(artifact_json(&out).to_string().contains("already absent"));
    assert!(!file_exists(&pool, &id).await);
}
