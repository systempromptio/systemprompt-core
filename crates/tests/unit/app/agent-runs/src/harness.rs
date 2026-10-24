//! Harness materialisation: digest and root checks, collisions, modes, and
//! the per-host target roots.

use std::collections::BTreeMap;
use std::path::PathBuf;

use systemprompt_agent_runs::execute::harness::{HarnessError, materialise, target_root};
use systemprompt_agent_runs::host::opencode::OpenCodeCli;
use systemprompt_agent_runs::{HarnessRef, HeadlessHost, RunLayout};
use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, SourceSnapshotId};
use systemprompt_models::managed::{
    ASSEMBLER_VERSION, AssetDigest, AssetFile, RevisionBundle, RevisionFiles, RevisionManifest,
};

fn file(bytes: &[u8], executable: bool) -> AssetFile {
    AssetFile {
        bytes: bytes.to_vec(),
        media_type: "text/plain".to_owned(),
        executable,
    }
}

fn bundle() -> RevisionBundle {
    let root = ResourceRevisionId::new("rev-root");
    let mut files = RevisionFiles::default();
    files
        .0
        .insert("AGENTS.md".to_owned(), file(b"# harness", false));
    files
        .0
        .insert("skills/count/run.sh".to_owned(), file(b"#!/bin/sh\n", true));
    let manifest = RevisionManifest::from_files(
        SourceSnapshotId::new("snap-1"),
        None,
        &files,
        BTreeMap::new(),
    )
    .expect("manifest");
    let assets = files
        .0
        .values()
        .map(|f| (AssetDigest::of(&f.bytes), f.bytes.clone()))
        .collect();
    RevisionBundle {
        schema_version: 1,
        assembler_version: ASSEMBLER_VERSION.to_owned(),
        root: root.clone(),
        revisions: BTreeMap::from([(root, manifest)]),
        assets,
    }
}

fn harness_ref(bundle: &RevisionBundle) -> HarnessRef {
    HarnessRef {
        resource_id: ManagedResourceId::new("res-1"),
        revision_id: bundle.root.clone(),
        bundle_digest: bundle.digest().expect("digest"),
    }
}

#[test]
fn materialise_writes_every_file_with_its_mode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bundle = bundle();
    let written = materialise(&harness_ref(&bundle), &bundle, dir.path()).expect("materialise");
    assert_eq!(written.len(), 2);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("AGENTS.md")).expect("agents"),
        "# harness"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join("skills/count/run.sh"))
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}

#[test]
fn materialise_refuses_a_digest_mismatch_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bundle = bundle();
    let mut wanted = harness_ref(&bundle);
    wanted.bundle_digest = AssetDigest::of(b"other");
    assert!(matches!(
        materialise(&wanted, &bundle, dir.path()),
        Err(HarnessError::Digest { .. })
    ));
    assert_eq!(std::fs::read_dir(dir.path()).expect("dir").count(), 0);
}

#[test]
fn materialise_refuses_another_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bundle = bundle();
    let mut wanted = harness_ref(&bundle);
    wanted.revision_id = ResourceRevisionId::new("rev-other");
    assert!(matches!(
        materialise(&wanted, &bundle, dir.path()),
        Err(HarnessError::Root { .. })
    ));
}

#[test]
fn materialise_refuses_a_tampered_asset() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut bundle = bundle();
    let wanted = harness_ref(&bundle);
    let digest = AssetDigest::of(b"# harness");
    bundle.assets.insert(digest, b"# tampered".to_vec());
    assert!(matches!(
        materialise(&wanted, &bundle, dir.path()),
        Err(HarnessError::Verify(_))
    ));
}

#[test]
fn opencode_harness_lands_in_the_global_config_tree() {
    let layout = RunLayout::new("/r");
    let request = crate::opencode::request();
    let root = target_root(OpenCodeCli::default().harness_targets(), &layout, &request);
    assert_eq!(root, PathBuf::from("/r/home/.config/opencode"));
}
