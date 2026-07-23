use std::fs;
use std::path::PathBuf;

use systemprompt_bridge::gateway::manifest::ArtifactEntry;
use systemprompt_bridge::ids::{LibraryArtifactId, Sha256Digest};
use systemprompt_bridge::integration::cowork_artifacts::sink::{
    ArtifactSink, FileSink, LIBRARY_STORE_FILE, STAGING_SUBDIR, SeedStaging, read_library_store,
};

fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bridge-artifact-sink-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

fn artifact(id: &str) -> ArtifactEntry {
    ArtifactEntry {
        id: LibraryArtifactId::try_new(id).unwrap(),
        name: format!("name of {id}"),
        description: "desc".into(),
        version: "1".into(),
        mcp_tools: vec![],
        content: "<p/>".into(),
        starred: false,
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
    }
}

#[test]
fn reading_a_missing_or_unparseable_store_yields_no_entries() {
    let dir = tempdir();
    assert!(read_library_store(&dir).is_empty(), "no store file at all");

    fs::write(dir.join(LIBRARY_STORE_FILE), "not json").unwrap();
    assert!(
        read_library_store(&dir).is_empty(),
        "an unparseable store degrades to empty"
    );
}

#[test]
fn reading_a_store_skips_entries_that_do_not_match_the_record_shape() {
    let dir = tempdir();
    FileSink
        .write(&dir, &[artifact("pipeline")])
        .expect("write store");
    let mut store: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&fs::read(dir.join(LIBRARY_STORE_FILE)).unwrap()).unwrap();
    store.insert("foreign".into(), serde_json::json!(42));
    fs::write(
        dir.join(LIBRARY_STORE_FILE),
        serde_json::to_vec(&store).unwrap(),
    )
    .unwrap();

    let read = read_library_store(&dir);
    assert_eq!(read.len(), 1, "the foreign entry is skipped");
    assert_eq!(
        read.get("pipeline").map(|s| s.name.as_str()),
        Some("name of pipeline")
    );
    assert_eq!(
        read.get("pipeline").and_then(|s| s.description.as_deref()),
        Some("desc")
    );
}

#[test]
fn each_sink_reports_materialisation_only_once_it_has_written() {
    let dir = tempdir();
    assert!(!FileSink.is_materialized(&dir));
    assert!(!SeedStaging.is_materialized(&dir));

    FileSink
        .write(&dir, &[artifact("pipeline")])
        .expect("write");
    assert!(FileSink.is_materialized(&dir));
    assert!(
        !SeedStaging.is_materialized(&dir),
        "the file sink does not create the staging dir"
    );

    fs::create_dir_all(dir.join(STAGING_SUBDIR)).unwrap();
    SeedStaging
        .write(&dir, &[artifact("pipeline")])
        .expect("write");
    assert!(SeedStaging.is_materialized(&dir));
}

#[test]
fn the_seed_staging_sink_refuses_an_unsafe_artifact_id() {
    let dir = tempdir();
    fs::create_dir_all(dir.join(STAGING_SUBDIR)).unwrap();
    SeedStaging
        .write(&dir, &[artifact("..-escape"), artifact("legit")])
        .expect("an unsafe id is skipped, not an error");

    let written: Vec<String> = fs::read_dir(dir.join(STAGING_SUBDIR))
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .collect();
    assert_eq!(
        written,
        vec!["legit.json".to_owned()],
        "only the safe id reaches disk"
    );
}
