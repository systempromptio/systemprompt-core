//! DB-backed tests for `core files show` covering id/path lookup and every
//! type-specific metadata conversion branch.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use serde_json::json;
use systemprompt_cli::core::files::{self, FilesCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: FilesCommands,
}

fn parse(args: &[&str]) -> FilesCommands {
    Harness::try_parse_from(std::iter::once("files").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

async fn seed_file(pool: &DbPool, metadata: serde_json::Value) -> (String, String) {
    let id = Uuid::new_v4();
    let path = format!("/uploads/show-test/{id}.bin");
    let url = format!("https://files.invalid/{id}");
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content, metadata, \
         user_id, session_id, trace_id, context_id) VALUES ($1, $2, $3, 'application/octet-stream', \
         42, true, $4, 'user-files-show', 'session-files-show', 'trace-files-show', \
         'context-files-show')",
    )
    .bind(id)
    .bind(&path)
    .bind(&url)
    .bind(metadata)
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    (id.to_string(), path)
}

#[tokio::test]
async fn show_by_id_renders_image_metadata() {
    let pool = pool().await;
    let metadata = json!({
        "checksums": {"md5": "abc", "sha256": "def"},
        "type_specific": {
            "type": "image",
            "width": 800,
            "height": 600,
            "alt_text": "alt",
            "description": "desc"
        }
    });
    let (id, _) = seed_file(&pool, metadata).await;
    files::execute(parse(&["show", &id]), &ctx(&pool))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_by_path_renders_document_metadata() {
    let pool = pool().await;
    let metadata = json!({
        "type_specific": {
            "type": "document",
            "title": "Doc",
            "author": "Author",
            "page_count": 3
        }
    });
    let (_, path) = seed_file(&pool, metadata).await;
    files::execute(parse(&["show", &path]), &ctx(&pool))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_renders_audio_metadata() {
    let pool = pool().await;
    let metadata = json!({
        "type_specific": {
            "type": "audio",
            "duration_seconds": 12.5,
            "sample_rate": 44100,
            "channels": 2
        }
    });
    let (id, _) = seed_file(&pool, metadata).await;
    files::execute(parse(&["show", &id]), &ctx(&pool))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_renders_video_metadata() {
    let pool = pool().await;
    let metadata = json!({
        "type_specific": {
            "type": "video",
            "width": 1920,
            "height": 1080,
            "duration_seconds": 30.0,
            "frame_rate": 24.0
        }
    });
    let (id, _) = seed_file(&pool, metadata).await;
    files::execute(parse(&["show", &id]), &ctx(&pool))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_renders_empty_metadata() {
    let pool = pool().await;
    let (id, _) = seed_file(&pool, json!({})).await;
    files::execute(parse(&["show", &id]), &ctx(&pool))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_missing_id_and_path_fail() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing_id = Uuid::new_v4().to_string();
    let err = files::execute(parse(&["show", &missing_id]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("not found"));

    let err = files::execute(parse(&["show", "/uploads/show-test/nope.bin"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("not found"));
}
