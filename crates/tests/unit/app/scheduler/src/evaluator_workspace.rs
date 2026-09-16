//! Production workspace materialization/integrity contracts without launching
//! a native client or registering synthetic native verification evidence.

use std::collections::BTreeMap;
use systemprompt_evaluation::experiments::resources::{CaseContent, Partition};
use systemprompt_identifiers::{ResourceRevisionId, SourceSnapshotId};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, RevisionBundle, RevisionFiles, RevisionManifest,
};
use systemprompt_scheduler::services::evaluator::supervisor::workspace::{
    WorkspaceDirectory, changed_workspace, install_case_fixtures, materialize_root,
    materialize_skills, workspace_state,
};

fn bundle(root: &str, files: &[(&str, &[u8], bool)]) -> RevisionBundle {
    let files = RevisionFiles(
        files
            .iter()
            .map(|(path, bytes, executable)| {
                (
                    (*path).to_owned(),
                    AssetFile {
                        bytes: bytes.to_vec(),
                        media_type: "text/plain".into(),
                        executable: *executable,
                    },
                )
            })
            .collect(),
    );
    let manifest = RevisionManifest::from_files(
        SourceSnapshotId::new("snapshot"),
        None,
        &files,
        BTreeMap::new(),
    )
    .unwrap();
    RevisionBundle {
        schema_version: 1,
        assembler_version: "managed-bundle-v1".into(),
        root: ResourceRevisionId::new(root),
        revisions: BTreeMap::from([(ResourceRevisionId::new(root), manifest)]),
        assets: files
            .0
            .values()
            .map(|f| (AssetDigest::of(&f.bytes), f.bytes.clone()))
            .collect(),
    }
}
fn case(fixtures: BTreeMap<String, String>) -> CaseContent {
    CaseContent {
        prompt: "Verify fixture".into(),
        expected_behavior: vec!["Preserve files".into()],
        fixtures,
        partition: Partition::Development,
        assertions: vec!["response_present".into()],
    }
}

#[test]
fn verified_bundle_installs_exact_root_and_skill_bytes_without_overwriting() {
    let root = tempfile::tempdir().unwrap();
    let bundle = bundle(
        "revision",
        &[
            ("SKILL.md", b"# retained", false),
            ("scripts/check.sh", b"#!/bin/sh\nexit 0\n", true),
        ],
    );
    let wire = bundle;
    materialize_root(&wire, &root.path().join("work")).unwrap();
    materialize_skills(&wire, &root.path().join("skills")).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("work/SKILL.md")).unwrap(),
        b"# retained"
    );
    assert_eq!(
        std::fs::read(root.path().join("skills/revision/scripts/check.sh")).unwrap(),
        b"#!/bin/sh\nexit 0\n"
    );
    assert!(
        materialize_root(&wire, &root.path().join("work")).is_err(),
        "materialization must not overwrite existing evidence"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.path().join("work/SKILL.md"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(root.path().join("work/scripts/check.sh"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}

#[test]
fn corrupted_or_incomplete_bundle_is_rejected_before_any_files_are_written() {
    let root = tempfile::tempdir().unwrap();
    let original = bundle("revision", &[("SKILL.md", b"retained", false)]);
    let mut bad_bytes = original.clone();
    *bad_bytes.assets.values_mut().next().unwrap() = b"tampered".to_vec();
    let mut missing = original.clone();
    missing.assets.clear();
    let mut undeclared = original;
    undeclared
        .assets
        .insert(AssetDigest::of(b"extra"), b"extra".to_vec());
    for invalid in [bad_bytes, missing, undeclared] {
        let destination = root.path().join("output");
        assert!(materialize_root(&invalid, &destination).is_err());
        assert!(!destination.exists());
    }
}

#[test]
fn skill_revision_identity_cannot_escape_the_installation_directory() {
    let root = tempfile::tempdir().unwrap();
    let invalid = bundle("../escape", &[("SKILL.md", b"payload", false)]);
    assert!(materialize_skills(&invalid, &root.path().join("skills")).is_err());
    assert!(!root.path().join("escape").exists());
}

#[test]
fn case_fixtures_validate_paths_and_refuse_collisions() {
    let root = tempfile::tempdir().unwrap();
    install_case_fixtures(
        &case(BTreeMap::from([(
            "data/input.txt".into(),
            "original".into(),
        )])),
        root.path(),
    )
    .unwrap();
    assert!(
        install_case_fixtures(
            &case(BTreeMap::from([(
                "data/input.txt".into(),
                "changed".into()
            )])),
            root.path()
        )
        .is_err()
    );
    assert_eq!(
        std::fs::read(root.path().join("data/input.txt")).unwrap(),
        b"original"
    );
    assert!(
        install_case_fixtures(
            &case(BTreeMap::from([("../escape".into(), "outside".into())])),
            root.path()
        )
        .is_err()
    );
}

#[test]
fn owned_workspace_is_removed_on_failure_without_adopting_existing_directories() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owned");
    {
        let guard = WorkspaceDirectory::create(path.clone()).unwrap();
        std::fs::write(guard.path().join("credential"), b"fixture-secret").unwrap();
        assert!(WorkspaceDirectory::create(path.clone()).is_err());
        assert!(
            path.join("credential").exists(),
            "failed acquisition cannot delete another workspace"
        );
        let mut invalid = bundle("invalid", &[("SKILL.md", b"invalid", false)]);
        invalid.schema_version = 0;
        assert!(materialize_root(&invalid, guard.path()).is_err());
    }
    assert!(
        !path.exists(),
        "early preparation failure drops owned credentials and files"
    );
}

#[test]
fn unchanged_files_are_excluded_and_evidence_limits_reject_excess_output() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("baseline"), b"before").unwrap();
    let baseline = workspace_state(root.path()).unwrap();
    assert!(
        changed_workspace(root.path(), &baseline)
            .unwrap()
            .is_empty()
    );
    std::fs::write(root.path().join("baseline"), b"after").unwrap();
    let changed = changed_workspace(root.path(), &baseline).unwrap();
    assert_eq!(changed["workspace/baseline"].bytes, b"after");
    for index in 0..253 {
        std::fs::write(root.path().join(format!("result-{index}")), b"x").unwrap();
    }
    assert!(
        changed_workspace(root.path(), &baseline).is_err(),
        "retained artifact count bound"
    );
}

#[test]
fn oversized_or_deep_workspace_is_rejected_before_unbounded_read() {
    let root = tempfile::tempdir().unwrap();
    let file = std::fs::File::create(root.path().join("large")).unwrap();
    file.set_len(65 * 1024 * 1024).unwrap();
    assert!(
        workspace_state(root.path()).is_err(),
        "sparse oversized file must not be loaded"
    );
    std::fs::remove_file(root.path().join("large")).unwrap();
    let mut nested = root.path().to_path_buf();
    for _ in 0..34 {
        nested = nested.join("child");
        std::fs::create_dir(&nested).unwrap();
    }
    assert!(workspace_state(root.path()).is_err(), "bounded recursion");
}

#[cfg(unix)]
#[test]
fn modes_and_directory_links_are_part_of_workspace_integrity() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("script");
    std::fs::write(&file, b"same bytes").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let baseline = workspace_state(root.path()).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
    let changed = changed_workspace(root.path(), &baseline).unwrap();
    assert!(changed["workspace/script"].executable);
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("linked")).unwrap();
    assert!(workspace_state(root.path()).is_err());
    assert!(
        install_case_fixtures(
            &case(BTreeMap::from([(
                "linked/secret".into(),
                "forbidden".into()
            )])),
            root.path()
        )
        .is_err()
    );
    assert!(!outside.path().join("secret").exists());
}

#[cfg(unix)]
#[test]
fn non_utf8_file_names_cannot_alias_retained_evidence_paths() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    let name = std::ffi::OsString::from_vec(vec![b'r', b'e', b's', 0xff]);
    std::fs::write(root.path().join(name), b"evidence").unwrap();
    let error = workspace_state(root.path()).expect_err("invalid UTF-8 name must be rejected");
    assert!(error.to_string().contains("UTF-8"));
    assert!(changed_workspace(root.path(), &BTreeMap::new()).is_err());
}

#[cfg(unix)]
#[test]
fn fallback_skill_bundle_rejects_linked_installation_parent() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let destination = root.path().join("skills");
    symlink(outside.path(), &destination).unwrap();
    let value = bundle("revision", &[("SKILL.md", b"private", false)]);
    assert!(materialize_skills(&value, &destination).is_err());
    assert!(
        !outside.path().join("revision").exists(),
        "reject before creating outside directories"
    );
}

#[test]
fn paired_workspaces_match_except_for_the_installed_skill_and_detect_configuration_drift() {
    let root = tempfile::tempdir().unwrap();
    let configuration = bundle("configuration", &[("project.txt", b"same project", false)]);
    let fixtures = case(BTreeMap::from([(
        "cases/input.txt".to_owned(),
        "same fixture".to_owned(),
    )]));
    let baseline = root.path().join("baseline");
    let candidate = root.path().join("candidate");
    for directory in [&baseline, &candidate] {
        materialize_root(&configuration, directory).unwrap();
        install_case_fixtures(&fixtures, directory).unwrap();
    }
    let before = workspace_state(&baseline).unwrap();
    assert_eq!(before, workspace_state(&candidate).unwrap());
    let baseline_skill = bundle("skill", &[("SKILL.md", b"# baseline", false)]);
    let candidate_skill = bundle("skill", &[("SKILL.md", b"# candidate", false)]);
    let baseline_skills = root.path().join("baseline-skills");
    let candidate_skills = root.path().join("candidate-skills");
    materialize_skills(&baseline_skill, &baseline_skills).unwrap();
    materialize_skills(&candidate_skill, &candidate_skills).unwrap();
    assert_ne!(
        workspace_state(&baseline_skills).unwrap(),
        workspace_state(&candidate_skills).unwrap()
    );
    assert_eq!(
        workspace_state(&baseline).unwrap(),
        workspace_state(&candidate).unwrap()
    );
    std::fs::write(candidate.join("project.txt"), b"changed configuration").unwrap();
    assert_ne!(before, workspace_state(&candidate).unwrap());
    assert_eq!(workspace_state(&baseline).unwrap(), before);
}
