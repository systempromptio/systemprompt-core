//! Authoring capture preserves immutable bytes and rejects unsafe filesystem
//! input.

use std::fs;
use systemprompt_marketplace::managed::capture_skills;

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("authoring fixture");
    let skill = root.path().join("skills/alpha");
    fs::create_dir_all(&skill).expect("skill directory");
    fs::write(
        skill.join("config.yaml"),
        "id: alpha\nname: Alpha\ndescription: Authoring fixture\nenabled: true\nfile: index.md\n",
    )
    .expect("config");
    fs::write(skill.join("index.md"), "# Original\n").expect("instructions");
    root
}

#[test]
fn authoring_capture_tracks_bytes_media_types_and_digest_without_executing_files() {
    let root = fixture();
    let script = root.path().join("skills/alpha/run.sh");
    fs::write(&script, "#!/bin/sh\nexit 99\n").unwrap();
    fs::write(root.path().join("skills/alpha/data.json"), "{}").unwrap();
    fs::write(root.path().join("skills/alpha/data.bin"), [0, 255]).unwrap();
    let first = capture_skills(root.path(), &["alpha".into()]).unwrap();
    let files = &first.skills()["alpha"].0;
    assert_eq!(files["data.json"].media_type, "application/json");
    assert_eq!(files["data.bin"].media_type, "application/octet-stream");
    assert_eq!(files["index.md"].media_type, "text/markdown");
    assert_eq!(files["config.yaml"].media_type, "application/yaml");
    assert_eq!(files["run.sh"].media_type, "text/plain");
    assert_eq!(files["run.sh"].bytes, b"#!/bin/sh\nexit 99\n");
    assert_eq!(
        first.tree_digest(),
        capture_skills(root.path(), &["alpha".into()])
            .unwrap()
            .tree_digest()
    );
    fs::write(root.path().join("skills/alpha/index.md"), "# Changed\n").unwrap();
    assert_ne!(
        first.tree_digest(),
        capture_skills(root.path(), &["alpha".into()])
            .unwrap()
            .tree_digest()
    );
    assert_eq!(first.skills()["alpha"].0["index.md"].bytes, b"# Original\n");
}

#[test]
fn authoring_capture_rejects_duplicate_traversal_and_invalid_configurations() {
    let root = fixture();
    for ids in [
        vec![],
        vec!["alpha".into(); 101],
        vec!["alpha".into(), "alpha".into()],
        vec!["../alpha".into()],
        vec![".".into()],
    ] {
        assert!(capture_skills(root.path(), &ids).is_err());
    }
    for config in [
        "id: other\nname: Alpha\ndescription: Authoring fixture\nenabled: true\nfile: index.md\n",
        "id: alpha\nname: Alpha\ndescription: Authoring fixture\nenabled: false\nfile: index.md\n",
        "[invalid yaml",
        "id: alpha\nname: Alpha\ndescription: Authoring fixture\nenabled: true\nfile: missing.md\n",
    ] {
        fs::write(root.path().join("skills/alpha/config.yaml"), config).unwrap();
        assert!(
            capture_skills(root.path(), &["alpha".into()]).is_err(),
            "{config}"
        );
    }
}

#[test]
fn authoring_capture_bounds_aggregate_bytes_file_count_and_depth() {
    let root = fixture();
    let skill = root.path().join("skills/alpha");
    fs::write(skill.join("oversized.bin"), vec![0; 8 * 1024 * 1024 + 1]).unwrap();
    assert!(capture_skills(root.path(), &["alpha".into()]).is_err());
    fs::remove_file(skill.join("oversized.bin")).unwrap();
    for index in 0..255 {
        fs::write(skill.join(format!("file-{index}")), "x").unwrap();
    }
    assert!(capture_skills(root.path(), &["alpha".into()]).is_err());
    let root = fixture();
    let deep = (0..34).fold(root.path().join("skills/alpha"), |path, _| {
        path.join("nested")
    });
    fs::create_dir_all(deep).unwrap();
    assert!(capture_skills(root.path(), &["alpha".into()]).is_err());
}

#[cfg(unix)]
#[test]
fn authoring_capture_rejects_symlink_roots_files_and_special_files() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = fixture();
    let script = root.path().join("skills/alpha/run.sh");
    fs::write(&script, "echo fixture\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o600)).unwrap();
    let plain = capture_skills(root.path(), &["alpha".into()]).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let executable = capture_skills(root.path(), &["alpha".into()]).unwrap();
    assert!(!plain.skills()["alpha"].0["run.sh"].executable);
    assert!(executable.skills()["alpha"].0["run.sh"].executable);
    assert_ne!(plain.tree_digest(), executable.tree_digest());
    let link = root.path().join("skills/alpha/link");
    symlink(&script, &link).unwrap();
    assert!(capture_skills(root.path(), &["alpha".into()]).is_err());
    fs::remove_file(link).unwrap();
    let socket =
        std::os::unix::net::UnixListener::bind(root.path().join("skills/alpha/socket")).unwrap();
    assert!(capture_skills(root.path(), &["alpha".into()]).is_err());
    drop(socket);
    let alias = root.path().join("alias");
    symlink(root.path(), &alias).unwrap();
    assert!(capture_skills(&alias, &["alpha".into()]).is_err());
}
