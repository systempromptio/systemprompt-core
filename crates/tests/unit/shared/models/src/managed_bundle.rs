use std::collections::BTreeMap;

use systemprompt_identifiers::{ResourceRevisionId, SourceSnapshotId};
use systemprompt_models::managed::{
    ASSEMBLER_VERSION, AssetDigest, AssetFile, DependencyRef, FileEntry, RevisionBundle,
    RevisionBundleError, RevisionFiles, RevisionManifest,
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

#[test]
fn orphan_asset_is_rejected_by_verify_and_every_canonical_export() {
    let mut candidate = bundle();
    candidate
        .assets
        .insert(AssetDigest::of(b"unreferenced"), b"unreferenced".to_vec());

    assert!(matches!(
        candidate.verify(),
        Err(RevisionBundleError::Integrity)
    ));
    assert!(matches!(
        candidate.canonical_bytes(),
        Err(RevisionBundleError::Integrity)
    ));
    assert!(matches!(
        candidate.digest(),
        Err(RevisionBundleError::Integrity)
    ));

    candidate.assets.remove(&AssetDigest::of(b"unreferenced"));
    candidate
        .verify()
        .expect("removing the orphan repairs the bundle");
}

#[test]
fn disconnected_revision_is_rejected_even_when_its_manifest_and_assets_are_valid() {
    let mut candidate = bundle();
    let disconnected = ResourceRevisionId::new("revision-disconnected");
    let valid_manifest = candidate
        .revisions
        .get(&candidate.root)
        .expect("root manifest")
        .clone();
    candidate
        .revisions
        .insert(disconnected.clone(), valid_manifest);

    assert!(matches!(
        candidate.verify(),
        Err(RevisionBundleError::Integrity)
    ));
    assert!(matches!(
        candidate.revision_files(&disconnected),
        Err(RevisionBundleError::Integrity)
    ));
}

#[test]
fn dependency_digest_mismatch_rejects_the_whole_closure_before_files_are_exposed() {
    let mut candidate = bundle();
    let dependency_id = ResourceRevisionId::new("revision-dependency");
    let dependency = candidate
        .revisions
        .get(&candidate.root)
        .expect("root manifest")
        .clone();
    candidate
        .revisions
        .insert(dependency_id.clone(), dependency);
    candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest")
        .dependencies
        .insert(
            "required-skill".to_owned(),
            DependencyRef {
                revision_id: dependency_id,
                digest: AssetDigest::of(b"forged dependency manifest"),
            },
        );

    assert!(matches!(
        candidate.verify(),
        Err(RevisionBundleError::Integrity)
    ));
    assert!(matches!(
        candidate.revision_files(&candidate.root),
        Err(RevisionBundleError::Integrity)
    ));

    let dependency_digest = candidate.revisions[&ResourceRevisionId::new("revision-dependency")]
        .digest()
        .expect("dependency manifest digest");
    candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest")
        .dependencies
        .get_mut("required-skill")
        .expect("dependency edge")
        .digest = dependency_digest;
    candidate
        .verify()
        .expect("restoring the exact dependency digest repairs the closure");
    let root_files = candidate
        .revision_files(&candidate.root)
        .expect("verified root files");
    assert_eq!(root_files.0["SKILL.md"].bytes, b"# skill");
}

#[test]
fn unsupported_revision_schema_is_rejected_until_the_manifest_is_repaired() {
    let mut candidate = bundle();
    candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest")
        .schema_version = 2;

    let error = candidate
        .verify()
        .expect_err("unknown revision schema must fail closed");
    assert!(
        matches!(&error, RevisionBundleError::Invalid(message) if message.contains("Unsupported revision manifest")),
        "{error}"
    );

    candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest")
        .schema_version = 1;
    candidate.verify().expect("supported schema repairs bundle");
}

#[test]
fn bundle_rejects_the_two_hundred_fifty_seventh_file_before_exposing_any_files() {
    let mut candidate = bundle();
    let template = candidate.revisions[&candidate.root].files["SKILL.md"].clone();
    let root = candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest");
    for index in 0..256 {
        root.files
            .insert(format!("generated/{index:03}.txt"), template.clone());
    }

    let error = candidate
        .verify()
        .expect_err("257 expanded files exceed the closure bound");
    assert!(
        matches!(&error, RevisionBundleError::Invalid(message) if message.contains("Bundle exceeds 256 files")),
        "{error}"
    );
    assert!(candidate.revision_files(&candidate.root).is_err());

    candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest")
        .files
        .remove("generated/255.txt");
    candidate.verify().expect("exactly 256 files are accepted");
    assert_eq!(
        candidate
            .revision_files(&candidate.root)
            .expect("bounded files are exposed")
            .0
            .len(),
        256
    );
}

#[test]
fn expanded_content_counts_repeated_assets_per_path_and_enforces_the_eight_mib_bound() {
    let mut candidate = bundle();
    let repeated = vec![b'x'; 1024 * 1024];
    let repeated_digest = AssetDigest::of(&repeated);
    candidate.assets.clear();
    candidate.assets.insert(repeated_digest.clone(), repeated);
    let root = candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest");
    root.files.clear();
    let repeated_entry = FileEntry {
        digest: repeated_digest,
        bytes: (1024 * 1024) as u64,
        media_type: "application/octet-stream".to_owned(),
        executable: false,
    };
    for index in 0..8 {
        root.files
            .insert(format!("payload/{index}.bin"), repeated_entry.clone());
    }

    candidate
        .verify()
        .expect("eight references to a shared 1 MiB asset are exactly 8 MiB expanded");
    assert_eq!(
        candidate.assets.values().map(Vec::len).sum::<usize>(),
        1024 * 1024
    );
    assert_eq!(
        candidate
            .revision_files(&candidate.root)
            .expect("bounded expanded files")
            .0
            .values()
            .map(|file| file.bytes.len())
            .sum::<usize>(),
        8 * 1024 * 1024
    );

    let one_byte = vec![b'!'];
    let one_byte_digest = AssetDigest::of(&one_byte);
    candidate.assets.insert(one_byte_digest.clone(), one_byte);
    candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest")
        .files
        .insert(
            "payload/overflow.bin".to_owned(),
            FileEntry {
                digest: one_byte_digest.clone(),
                bytes: 1,
                media_type: "application/octet-stream".to_owned(),
                executable: false,
            },
        );

    let error = candidate
        .verify()
        .expect_err("8 MiB plus one expanded byte must be rejected");
    assert!(
        matches!(&error, RevisionBundleError::Invalid(message) if message.contains("Bundle exceeds 8 MiB expanded content")),
        "{error}"
    );
    assert!(candidate.revision_files(&candidate.root).is_err());

    candidate
        .revisions
        .get_mut(&candidate.root)
        .expect("root manifest")
        .files
        .remove("payload/overflow.bin");
    candidate.assets.remove(&one_byte_digest);
    candidate
        .verify()
        .expect("removing the extra expanded byte restores the exact limit");
}
