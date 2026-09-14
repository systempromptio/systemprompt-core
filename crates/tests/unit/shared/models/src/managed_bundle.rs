use std::collections::BTreeMap;

use systemprompt_identifiers::{ResourceRevisionId, SourceSnapshotId};
use systemprompt_models::managed::{
    ASSEMBLER_VERSION, AssetDigest, AssetFile, RevisionBundle, RevisionBundleError, RevisionFiles,
    RevisionManifest,
};

fn bundle() -> RevisionBundle {
    let root = ResourceRevisionId::new("revision-root");
    let mut files = RevisionFiles::default();
    files.0.insert(
        "SKILL.md".to_owned(),
        AssetFile {
            bytes: b"# skill".to_vec(),
            media_type: "text/markdown".to_owned(),
            executable: false,
        },
    );
    let manifest = RevisionManifest::from_files(
        SourceSnapshotId::new("snapshot-1"),
        None,
        &files,
        BTreeMap::new(),
    )
    .expect("manifest");
    let mut assets = BTreeMap::new();
    assets.insert(AssetDigest::of(b"# skill"), b"# skill".to_vec());
    let mut revisions = BTreeMap::new();
    revisions.insert(root.clone(), manifest);
    RevisionBundle {
        schema_version: 1,
        assembler_version: ASSEMBLER_VERSION.to_owned(),
        root,
        revisions,
        assets,
    }
}

#[test]
fn a_well_formed_bundle_verifies_and_round_trips_through_json() {
    let bundle = bundle();
    bundle.verify().expect("verify");
    let json = serde_json::to_value(&bundle).expect("serialise");
    let decoded: RevisionBundle = serde_json::from_value(json).expect("deserialise");
    assert_eq!(decoded, bundle);
    assert_eq!(
        decoded.digest().expect("digest"),
        bundle.digest().expect("digest")
    );
}

#[test]
fn tampered_asset_bytes_fail_integrity() {
    let mut bundle = bundle();
    for bytes in bundle.assets.values_mut() {
        bytes.push(b'!');
    }
    assert!(matches!(
        bundle.verify(),
        Err(RevisionBundleError::Integrity)
    ));
}

#[test]
fn unsupported_assembler_version_is_invalid() {
    let mut bundle = bundle();
    bundle.assembler_version = "managed-bundle-v0".to_owned();
    assert!(matches!(
        bundle.verify(),
        Err(RevisionBundleError::Invalid(_))
    ));
}

#[test]
fn revision_files_for_an_unknown_revision_is_missing_revision() {
    let bundle = bundle();
    let unknown = ResourceRevisionId::new("revision-unknown");
    assert!(matches!(
        bundle.revision_files(&unknown),
        Err(RevisionBundleError::MissingRevision(id)) if id == unknown
    ));
}

#[test]
fn content_digest_ignores_revision_history() {
    let bundle = bundle();
    let mut rebased = bundle.clone();
    let root = rebased.root.clone();
    let manifest = rebased.revisions.get_mut(&root).expect("root manifest");
    manifest.snapshot_id = SourceSnapshotId::new("snapshot-2");
    manifest.parent_id = Some(ResourceRevisionId::new("revision-parent"));
    assert_ne!(
        bundle.digest().expect("digest"),
        rebased.digest().expect("digest")
    );
    assert_eq!(
        bundle.content_digest().expect("content digest"),
        rebased.content_digest().expect("content digest")
    );
}

fn files(entries: &[(&str, &[u8], bool)]) -> RevisionFiles {
    let mut files = RevisionFiles::default();
    for (path, bytes, executable) in entries {
        files.0.insert(
            (*path).to_owned(),
            AssetFile {
                bytes: bytes.to_vec(),
                media_type: "text/plain".to_owned(),
                executable: *executable,
            },
        );
    }
    files
}

#[test]
fn same_content_requires_identical_paths_bytes_and_executable_bits() {
    let retained = files(&[
        ("SKILL.md", b"# skill", false),
        ("run.sh", b"#!/bin/sh", true),
    ]);

    assert!(retained.same_content(&files(&[
        ("SKILL.md", b"# skill", false),
        ("run.sh", b"#!/bin/sh", true)
    ])));
    assert!(
        !retained.same_content(&files(&[("SKILL.md", b"# skill", false)])),
        "a missing path is a difference"
    );
    assert!(
        !retained.same_content(&files(&[
            ("SKILL.md", b"# skill", false),
            ("run.sh", b"#!/bin/sh", false)
        ])),
        "an executable bit is content"
    );
    assert!(
        !retained.same_content(&files(&[
            ("SKILL.md", b"# skill", false),
            ("extra.md", b"", false),
            ("run.sh", b"#!/bin/sh", true)
        ])),
        "an extra path is a difference"
    );
    assert!(
        !RevisionFiles::default().same_content(&retained),
        "an empty import never matches a retained revision"
    );
}
