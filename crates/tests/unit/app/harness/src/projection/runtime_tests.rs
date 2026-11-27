use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, InstallationPlanFile, ReadbackStatus,
};

use serde_json::json;
use systemprompt_device::protocol::names::sha256_hex;
use systemprompt_harness::projection::{Applier, BundleFiles, MaterializeError, RUNTIME_LEDGER};

use crate::projection::fixtures::{self, skill};

const SKILL_DIR: &str = "alpha-notes";

fn file(path: &str, text: &str, executable: bool) -> InstallationPlanFile {
    InstallationPlanFile {
        path: path.to_owned(),
        bytes: text.as_bytes().to_vec(),
        executable,
    }
}

fn installation(files: Vec<InstallationPlanFile>) -> ConsumerInstallationPlan {
    let mut plan: ConsumerInstallationPlan = serde_json::from_value(json!({
        "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
        "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
        "revision_id": "802c1429-27ed-4274-afd9-51e2ed79ea79",
        "generation": 1,
        "bundle_digest": sha256_hex(b"bundle"),
        "host": systemprompt_models::bridge::host::HostKind::Harness.as_str(),
        "canonical_files": [],
        "runtime_files": []
    }))
    .unwrap();
    plan.runtime_files = files;
    plan
}

fn base() -> Vec<InstallationPlanFile> {
    vec![
        file(
            "SKILL.md",
            "---\nname: alpha_notes\ndescription: From the bundle.\n---\n# Alpha notes\n",
            false,
        ),
        file("config.yaml", "id: alpha_notes\n", false),
        file("references/guide.md", "# Guide\n", false),
    ]
}

fn setup(root: &Path) -> (Applier, PathBuf, String) {
    let applier = fixtures::applier(root);
    let manifest = fixtures::manifest(json!({ "skills": [skill("alpha_notes", json!({}))] }));
    applier
        .apply(&fixtures::projected(&manifest, &BundleFiles::new()))
        .unwrap();
    let dir = applier.config_dir().join("skills").join(SKILL_DIR);
    let projected = fs::read_to_string(dir.join("SKILL.md")).unwrap();
    (applier, dir, projected)
}

#[test]
fn writes_beside_skill_md_reads_back_and_restores_the_projection() {
    let root = tempfile::tempdir().unwrap();
    let (applier, dir, projected) = setup(root.path());
    let mut files = base();
    files.push(file("scripts/run.sh", "#!/bin/sh\necho ok\n", true));
    let plan = installation(files.clone());
    let readback = applier.materialize(SKILL_DIR, &plan).unwrap();
    assert_eq!(readback.len(), files.len());
    for (read, planned) in readback.iter().zip(&files) {
        assert_eq!(read.path, planned.path);
        assert_eq!(read.content_check, ReadbackStatus::Verified);
        assert_eq!(read.digest.as_str(), sha256_hex(&planned.bytes));
        assert_eq!(read.bytes, u64::try_from(planned.bytes.len()).unwrap());
        assert_ne!(read.mode_check, ReadbackStatus::Mismatch);
    }
    assert_eq!(
        fs::read_to_string(dir.join("config.yaml")).unwrap(),
        "id: alpha_notes\n"
    );
    assert_eq!(fs::read_to_string(dir.join("SKILL.md")).unwrap(), projected);
    assert_eq!(
        fs::read_to_string(dir.join(RUNTIME_LEDGER)).unwrap(),
        r#"["SKILL.md","config.yaml","references/guide.md","scripts/run.sh"]"#
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |path: &str| fs::metadata(dir.join(path)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("config.yaml"), 0o644);
        assert_eq!(mode("scripts/run.sh"), 0o755);
        assert_eq!(mode(RUNTIME_LEDGER), 0o600);
    }
    assert!(applier.inspect(SKILL_DIR, &plan));
    fs::write(dir.join("config.yaml"), "changed").unwrap();
    assert!(!applier.inspect(SKILL_DIR, &plan));
}

#[test]
fn a_new_plan_removes_what_the_previous_listed_and_nothing_else() {
    let root = tempfile::tempdir().unwrap();
    let (applier, dir, _) = setup(root.path());
    let mut first = base();
    first.push(file("references/old.md", "old", false));
    applier
        .materialize(SKILL_DIR, &installation(first))
        .unwrap();
    fs::write(dir.join("notes-of-the-user.md"), "mine").unwrap();
    let mut second = base();
    second.push(file("references/new.md", "new", false));
    applier
        .materialize(SKILL_DIR, &installation(second))
        .unwrap();
    assert!(!dir.join("references/old.md").exists());
    assert_eq!(
        fs::read_to_string(dir.join("references/new.md")).unwrap(),
        "new"
    );
    assert_eq!(
        fs::read_to_string(dir.join("notes-of-the-user.md")).unwrap(),
        "mine"
    );
}

#[test]
fn runtime_files_go_with_the_skill_directory() {
    let root = tempfile::tempdir().unwrap();
    let (applier, dir, _) = setup(root.path());
    let plan = installation(base());
    applier.materialize(SKILL_DIR, &plan).unwrap();
    applier.clear().unwrap();
    assert!(!dir.exists());
    assert!(!applier.inspect(SKILL_DIR, &plan));
}

#[test]
fn refuses_bad_plans_and_unmanaged_skills() {
    let root = tempfile::tempdir().unwrap();
    let (applier, dir, _) = setup(root.path());
    let without_skill: Vec<InstallationPlanFile> = base()
        .into_iter()
        .filter(|f| f.path != "SKILL.md")
        .collect();
    let refused = |files: Vec<InstallationPlanFile>| {
        matches!(
            applier.materialize(SKILL_DIR, &installation(files)),
            Err(MaterializeError::InvalidPlan { .. })
        )
    };
    assert!(refused(without_skill));
    assert!(refused(Vec::new()));
    let bad_paths = [
        "../escape.md",
        "a/./b.md",
        "a//b.md",
        "/abs.md",
        "c:evil.md",
        "a\\b.md",
        RUNTIME_LEDGER,
    ];
    for bad in bad_paths {
        let mut files = base();
        files.push(file(bad, "x", false));
        assert!(refused(files), "{bad}");
    }
    let mut twice = base();
    twice.push(file("config.yaml", "again", false));
    assert!(refused(twice));
    assert!(matches!(
        applier.materialize("not-managed", &installation(base())),
        Err(MaterializeError::NotManaged { .. })
    ));
    assert!(!applier.inspect("not-managed", &installation(base())));
    assert!(!root.path().join("config/skills/escape.md").exists());
    assert!(!dir.join(RUNTIME_LEDGER).exists());
}

#[cfg(unix)]
#[test]
fn never_writes_through_a_link() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let (applier, dir, _) = setup(root.path());
    std::os::unix::fs::symlink(outside.path(), dir.join("references")).unwrap();
    let result = applier.materialize(SKILL_DIR, &installation(base()));
    assert!(matches!(result, Err(MaterializeError::Secure { .. })));
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}
