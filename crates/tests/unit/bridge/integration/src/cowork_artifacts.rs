//! Tests for the Cowork artifacts emitter: content-hash idempotency in
//! `emit::write_artifacts`, foreign-key preservation in `FileSink`, and the
//! unsafe-id guard in `SeedStaging`.

use systemprompt_bridge::gateway::manifest::ArtifactEntry;
use systemprompt_bridge::ids::{LibraryArtifactId, PluginId, Sha256Digest};
use systemprompt_bridge::integration::cowork_artifacts::emit::{
    active_sink, artifacts_version, write_artifacts,
};
use systemprompt_bridge::integration::cowork_artifacts::sink::{
    ArtifactSink, FileSink, LIBRARY_STORE_FILE, STAGING_SUBDIR, SeedStaging,
};
use tempfile::tempdir;

fn artifact(id: &str, version: &str, content: &str) -> ArtifactEntry {
    ArtifactEntry {
        id: LibraryArtifactId::try_new(id).unwrap(),
        name: format!("Artifact {id}"),
        description: format!("desc {id}"),
        version: version.to_owned(),
        plugin_id: PluginId::try_new("plugin-a").unwrap(),
        mcp_tools: vec!["tool-one".into()],
        content: content.to_owned(),
        starred: true,
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
    }
}

#[test]
fn artifacts_version_is_order_independent() {
    let a = artifact("a", "1", "<p>a</p>");
    let b = artifact("b", "1", "<p>b</p>");
    let v1 = artifacts_version(&[a.clone(), b.clone()]);
    let v2 = artifacts_version(&[b, a]);
    assert_eq!(v1, v2);
}

#[test]
fn artifacts_version_changes_with_entry_version() {
    let v1 = artifacts_version(&[artifact("a", "1", "x")]);
    let v2 = artifacts_version(&[artifact("a", "2", "x")]);
    assert_ne!(v1, v2);
}

#[test]
fn write_artifacts_rewrites_when_store_removed_externally() {
    let temp = tempdir().unwrap();
    let dir = temp.path().join("cowork_artifacts");
    let set = vec![artifact("doc-1", "1", "x")];

    write_artifacts(&dir, active_sink(), &set).unwrap();
    std::fs::remove_file(dir.join(LIBRARY_STORE_FILE)).unwrap();

    write_artifacts(&dir, active_sink(), &set).unwrap();
    assert!(dir.join(LIBRARY_STORE_FILE).is_file());
}

#[test]
fn file_sink_is_materialized_tracks_store_file() {
    let temp = tempdir().unwrap();
    let dir = temp.path().to_path_buf();
    assert!(!FileSink.is_materialized(&dir));
    FileSink
        .write(&dir, &[artifact("doc-1", "1", "x")])
        .unwrap();
    assert!(FileSink.is_materialized(&dir));
}

#[test]
fn seed_staging_skips_unsafe_artifact_ids() {
    let temp = tempdir().unwrap();
    let dir = temp.path().to_path_buf();

    SeedStaging
        .write(&dir, &[artifact("../escape", "1", "x")])
        .unwrap();

    assert!(
        !dir.join(STAGING_SUBDIR).join("../escape.json").exists(),
        "unsafe id must not be written"
    );
}
