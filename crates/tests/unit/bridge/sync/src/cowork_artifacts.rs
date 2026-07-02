use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::gateway::manifest::ArtifactEntry;
use systemprompt_bridge::ids::{LibraryArtifactId, PluginId, Sha256Digest};
use systemprompt_bridge::integration::cowork_artifacts::emit::{
    artifacts_version, remove_dir, write_artifacts,
};
use systemprompt_bridge::integration::cowork_artifacts::sink::{
    ArtifactSink, FileSink, LIBRARY_STORE_FILE, STAGING_SUBDIR, SeedStaging,
};

fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bridge-artifacts-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn artifact(id: &str, version: &str) -> ArtifactEntry {
    ArtifactEntry {
        id: LibraryArtifactId::try_new(id).unwrap(),
        name: id.to_owned(),
        description: "desc".into(),
        version: version.to_owned(),
        plugin_id: PluginId::try_new("sfdc").unwrap(),
        mcp_tools: vec!["mcp__salesforce__query".to_owned()],
        content: format!("<table id=\"{id}\"></table>"),
        starred: true,
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
    }
}

fn version_marker(dir: &Path) -> Option<String> {
    let bytes = fs::read(dir.join("version.json")).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("version")?.as_str().map(str::to_owned)
}

#[test]
fn file_sink_writes_store_and_version_marker() {
    let dir = tempdir();
    let store = dir.join("cowork_artifacts");
    let artifacts = vec![artifact("pipeline", "1"), artifact("accounts", "1")];

    write_artifacts(&store, &FileSink, &artifacts).expect("write artifacts");

    let library: serde_json::Value =
        serde_json::from_slice(&fs::read(store.join(LIBRARY_STORE_FILE)).unwrap()).unwrap();
    assert!(library.get("pipeline").is_some());
    assert!(library.get("accounts").is_some());
    assert_eq!(library["pipeline"]["isStarred"], serde_json::json!(true));
    assert_eq!(
        library["pipeline"]["mcpTools"],
        serde_json::json!(["mcp__salesforce__query"])
    );
    assert_eq!(
        version_marker(&store).as_deref(),
        Some(artifacts_version(&artifacts).as_str())
    );
}

#[test]
fn file_sink_second_run_is_noop_when_unchanged() {
    let dir = tempdir();
    let store = dir.join("cowork_artifacts");
    let artifacts = vec![artifact("pipeline", "1")];

    write_artifacts(&store, &FileSink, &artifacts).expect("first write");
    let marker = store.join(LIBRARY_STORE_FILE);
    let mtime1 = fs::metadata(&marker).unwrap().modified().unwrap();

    write_artifacts(&store, &FileSink, &artifacts).expect("second write");
    let mtime2 = fs::metadata(&marker).unwrap().modified().unwrap();

    assert_eq!(mtime1, mtime2, "unchanged set must not rewrite the store");
}

#[test]
fn file_sink_upserts_on_version_bump() {
    let dir = tempdir();
    let store = dir.join("cowork_artifacts");

    write_artifacts(&store, &FileSink, &[artifact("pipeline", "1")]).expect("v1");
    let v1 = version_marker(&store);

    write_artifacts(&store, &FileSink, &[artifact("pipeline", "2")]).expect("v2");
    let v2 = version_marker(&store);

    assert_ne!(v1, v2, "version bump changes the marker");
    let library: serde_json::Value =
        serde_json::from_slice(&fs::read(store.join(LIBRARY_STORE_FILE)).unwrap()).unwrap();
    assert_eq!(library["pipeline"]["version"], serde_json::json!("2"));
}

#[test]
fn file_sink_preserves_foreign_entries() {
    let dir = tempdir();
    let store = dir.join("cowork_artifacts");
    fs::create_dir_all(&store).unwrap();
    fs::write(
        store.join(LIBRARY_STORE_FILE),
        serde_json::to_vec(&serde_json::json!({ "foreign": { "keep": true } })).unwrap(),
    )
    .unwrap();

    write_artifacts(&store, &FileSink, &[artifact("pipeline", "1")]).expect("write");

    let library: serde_json::Value =
        serde_json::from_slice(&fs::read(store.join(LIBRARY_STORE_FILE)).unwrap()).unwrap();
    assert!(
        library.get("foreign").is_some(),
        "unmanaged entry preserved"
    );
    assert!(library.get("pipeline").is_some(), "managed entry upserted");
}

#[test]
fn seed_staging_writes_one_record_per_artifact() {
    let dir = tempdir();
    let store = dir.join("cowork_artifacts");
    let artifacts = vec![artifact("pipeline", "1"), artifact("accounts", "1")];

    write_artifacts(&store, &SeedStaging, &artifacts).expect("write artifacts");

    let staging = store.join(STAGING_SUBDIR);
    assert!(staging.join("pipeline.json").is_file());
    assert!(staging.join("accounts.json").is_file());
    assert!(SeedStaging.is_materialized(&store));
}

#[test]
fn empty_set_removes_the_store() {
    let dir = tempdir();
    let store = dir.join("cowork_artifacts");
    write_artifacts(&store, &FileSink, &[artifact("pipeline", "1")]).expect("write");
    assert!(store.exists());

    write_artifacts(&store, &FileSink, &[]).expect("empty write");
    assert!(!store.exists(), "empty artifact set removes the store dir");
}

#[test]
fn remove_dir_is_idempotent() {
    let dir = tempdir();
    let store = dir.join("cowork_artifacts");
    remove_dir(&store).expect("remove missing dir is a no-op");
    write_artifacts(&store, &FileSink, &[artifact("pipeline", "1")]).expect("write");
    remove_dir(&store).expect("remove existing dir");
    assert!(!store.exists());
}
