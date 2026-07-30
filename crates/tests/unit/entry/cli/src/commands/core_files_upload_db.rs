//! App-context tests driving `core files upload` against a real database and
//! the bootstrap fixture's storage root.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use sha2::{Digest, Sha256};
use systemprompt_cli::core::files::upload::{self, UploadArgs};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use tempfile::TempDir;

async fn pool(database_url: &str) -> DbPool {
    fixture_db_pool(database_url).await.unwrap()
}

fn ctx(pool: &DbPool, database_url: &str) -> CommandContext {
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(pool, database_url).unwrap(),
    )
}

fn args(file_path: PathBuf, context: &ContextId) -> UploadArgs {
    UploadArgs {
        file_path,
        context: context.as_str().to_owned(),
        user: None,
        session: None,
        ai: false,
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut acc, b| {
            acc.push_str(&format!("{b:02x}"));
            acc
        })
}

#[tokio::test]
async fn uploads_a_text_file_and_reports_size_mime_and_checksum() {
    let boot = ensure_test_bootstrap();
    let pool = pool(&boot.database_url).await;
    let ctx = ctx(&pool, &boot.database_url);

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("notes.txt");
    let body = b"coverage matters\n";
    std::fs::write(&path, body).unwrap();

    let context_id = ContextId::generate();
    let output = upload::execute(args(path, &context_id), &ctx)
        .await
        .unwrap();

    let value = serde_json::to_value(output.artifact()).unwrap();
    let rendered = serde_json::to_string(&value).unwrap();

    assert!(
        rendered.contains(&hex_sha256(body)),
        "sha256 of the uploaded bytes should appear in the output: {rendered}"
    );
    assert!(
        rendered.contains("text/plain"),
        "mime type should be detected from the extension: {rendered}"
    );
    assert!(
        rendered.contains(&body.len().to_string()),
        "byte length should be reported: {rendered}"
    );
    assert!(
        rendered.contains(context_id.as_str()),
        "the stored path should be scoped to the context: {rendered}"
    );
    assert!(
        !rendered.contains("notes.txt"),
        "stored objects are renamed to the file id, not the caller's filename: {rendered}"
    );
    assert!(
        rendered.contains(".txt"),
        "the extension should survive the rename: {rendered}"
    );
}

#[tokio::test]
async fn uploads_binary_content_without_corrupting_the_checksum() {
    let boot = ensure_test_bootstrap();
    let pool = pool(&boot.database_url).await;
    let ctx = ctx(&pool, &boot.database_url);

    // A PNG header followed by every byte value: an allowed mime type whose
    // payload is genuinely binary, so a base64 round-trip bug would show up in
    // the digest.
    let mut body: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    body.extend(0u8..=255);

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("blob.png");
    std::fs::write(&path, &body).unwrap();

    let context_id = ContextId::generate();
    let output = upload::execute(args(path, &context_id), &ctx)
        .await
        .unwrap();

    let rendered =
        serde_json::to_string(&serde_json::to_value(output.artifact()).unwrap()).unwrap();
    assert!(
        rendered.contains(&hex_sha256(&body)),
        "base64 round trip must not alter the digest: {rendered}"
    );
    assert!(
        rendered.contains(&body.len().to_string()),
        "size should be the raw byte length, not the base64 length: {rendered}"
    );
    assert!(
        rendered.contains("image/png"),
        "mime should come from the extension: {rendered}"
    );
}

#[tokio::test]
async fn disallowed_mime_type_is_rejected() {
    let boot = ensure_test_bootstrap();
    let pool = pool(&boot.database_url).await;
    let ctx = ctx(&pool, &boot.database_url);

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("payload.bin");
    std::fs::write(&path, b"\x00\x01\x02opaque").unwrap();

    let context_id = ContextId::generate();
    let err = upload::execute(args(path, &context_id), &ctx)
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("application/octet-stream") && msg.contains("not allowed"),
        "an unlisted mime type must be refused by the upload policy: {msg}"
    );
}

#[tokio::test]
async fn empty_file_uploads_with_the_empty_digest() {
    let boot = ensure_test_bootstrap();
    let pool = pool(&boot.database_url).await;
    let ctx = ctx(&pool, &boot.database_url);

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("empty.txt");
    std::fs::write(&path, b"").unwrap();

    let context_id = ContextId::generate();
    let output = upload::execute(args(path, &context_id), &ctx)
        .await
        .unwrap();

    let rendered =
        serde_json::to_string(&serde_json::to_value(output.artifact()).unwrap()).unwrap();
    assert!(
        rendered.contains(&hex_sha256(b"")),
        "empty input still yields the well-known sha256: {rendered}"
    );
}

#[tokio::test]
async fn missing_file_reports_the_offending_path() {
    let boot = ensure_test_bootstrap();
    let pool = pool(&boot.database_url).await;
    let ctx = ctx(&pool, &boot.database_url);

    let context_id = ContextId::generate();
    let missing = PathBuf::from("/nonexistent/definitely-not-here.txt");
    let err = upload::execute(args(missing, &context_id), &ctx)
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(msg.contains("File not found"), "unexpected error: {msg}");
    assert!(
        msg.contains("definitely-not-here.txt"),
        "the path should be echoed back: {msg}"
    );
}

#[tokio::test]
async fn attaches_the_supplied_user_and_session() {
    let boot = ensure_test_bootstrap();
    let pool = pool(&boot.database_url).await;
    let ctx = ctx(&pool, &boot.database_url);

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("attributed.md");
    std::fs::write(&path, b"# heading\n").unwrap();

    let context_id = ContextId::generate();
    let mut a = args(path, &context_id);
    a.user = Some("upload-user".to_owned());
    a.session = Some("upload-session".to_owned());
    a.ai = true;

    let output = upload::execute(a, &ctx).await.unwrap();
    let rendered =
        serde_json::to_string(&serde_json::to_value(output.artifact()).unwrap()).unwrap();
    assert!(
        rendered.contains("text/markdown") || rendered.contains("text/"),
        "markdown should map to a text mime: {rendered}"
    );
}

#[test]
fn detect_mime_type_maps_known_extensions() {
    for (name, expected) in [
        ("a.txt", "text/plain"),
        ("a.json", "application/json"),
        ("a.png", "image/png"),
    ] {
        let got = upload::detect_mime_type(&PathBuf::from(name));
        assert_eq!(got, expected, "unexpected mime for {name}");
    }
}

#[test]
fn detect_mime_type_falls_back_for_unknown_extensions() {
    let got = upload::detect_mime_type(&PathBuf::from("mystery.zzzz"));
    assert!(
        !got.is_empty(),
        "an unknown extension must still yield some mime type"
    );
}
