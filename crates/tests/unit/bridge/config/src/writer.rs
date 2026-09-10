use std::fs;

use systemprompt_bridge::config::write;
use tempfile::TempDir;

fn write_file(dir: &TempDir, body: &str) -> std::path::PathBuf {
    let path = dir.path().join("bridge.toml");
    fs::write(&path, body).expect("seed config");
    path
}

#[test]
fn setting_pinned_pubkey_keeps_sections_that_follow_sync() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_file(
        &dir,
        "gateway_url = \"https://gateway.example.com\"\n\n\
         [sync]\npinned_pubkey = \"old\"\n\n\
         [claude]\nauth_scheme = \"bearer\"\nmodels = [\"claude-opus-4\"]\n\n\
         [session]\nenabled = true\n",
    );

    write::edit_file(&path, |doc| {
        write::set(doc, &["sync", "pinned_pubkey"], "new")?;
        Ok(())
    })
    .expect("write pubkey");

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("pinned_pubkey = \"new\""), "{after}");
    assert!(
        after.contains("[claude]"),
        "claude section dropped: {after}"
    );
    assert!(after.contains("auth_scheme = \"bearer\""), "{after}");
    assert!(
        after.contains("[session]"),
        "session section dropped: {after}"
    );
    assert!(after.contains("enabled = true"), "{after}");
}

#[test]
fn editing_preserves_comments_and_unknown_keys() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_file(
        &dir,
        "# operator note: do not remove\n\
         gateway_url = \"https://gateway.example.com\"\n\
         a_key_this_build_does_not_know = 7\n\n\
         [session]\n# keep sessions off on this fleet\nenabled = false\n",
    );

    write::edit_file(&path, |doc| {
        write::set(doc, &["session", "enabled"], true)?;
        Ok(())
    })
    .expect("write toggle");

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("# operator note: do not remove"), "{after}");
    assert!(
        after.contains("# keep sessions off on this fleet"),
        "{after}"
    );
    assert!(
        after.contains("a_key_this_build_does_not_know = 7"),
        "{after}"
    );
    assert!(after.contains("enabled = true"), "{after}");
}

#[test]
fn editing_a_malformed_file_reports_rather_than_overwriting() {
    let dir = TempDir::new().expect("tempdir");
    let body = "gateway_url = \"https://gateway.example.com\n[sync\n";
    let path = write_file(&dir, body);

    let err = write::edit_file(&path, |doc| {
        write::set(doc, &["session", "enabled"], true)?;
        Ok(())
    })
    .expect_err("malformed config must not be silently rewritten");

    assert!(
        matches!(err, write::ConfigWriteError::Malformed { .. }),
        "unexpected error: {err}"
    );
    assert_eq!(fs::read_to_string(&path).expect("read back"), body);
}

#[test]
fn set_if_absent_leaves_an_existing_value_alone() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_file(&dir, "gateway_url = \"https://operator.example.com\"\n");

    write::edit_file(&path, |doc| {
        write::set_if_absent(doc, &["gateway_url"], "https://installer.example.com")?;
        Ok(())
    })
    .expect("write");

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("https://operator.example.com"), "{after}");
    assert!(!after.contains("installer.example.com"), "{after}");
}

#[test]
fn set_if_absent_writes_into_an_empty_file() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_file(&dir, "");

    write::edit_file(&path, |doc| {
        write::set_if_absent(doc, &["gateway_url"], "https://installer.example.com")?;
        Ok(())
    })
    .expect("write");

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("https://installer.example.com"), "{after}");
}

#[test]
fn removing_a_credential_section_leaves_the_rest_intact() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_file(
        &dir,
        "gateway_url = \"https://gateway.example.com\"\n\n\
         [pat]\nfile = \"/etc/bridge/pat.token\"\n\n\
         [sync]\npinned_pubkey = \"abc\"\n",
    );

    write::edit_file(&path, |doc| {
        write::remove(doc, &["pat"])?;
        write::set(doc, &["session", "enabled"], true)?;
        Ok(())
    })
    .expect("swap credential section");

    let after = fs::read_to_string(&path).expect("read back");
    assert!(!after.contains("[pat]"), "pat section survived: {after}");
    assert!(after.contains("enabled = true"), "{after}");
    assert!(after.contains("pinned_pubkey = \"abc\""), "{after}");
}

#[test]
fn incompatible_parent_is_an_error_and_preserves_the_file() {
    let dir = TempDir::new().unwrap();
    let path = write_file(&dir, "sync = 42\n");
    let error = write::edit_file(&path, |doc| {
        write::set(doc, &["sync", "trust", "key"], "key")
    })
    .unwrap_err();
    assert!(matches!(error, write::ConfigWriteError::InvalidPath { .. }));
    assert_eq!(fs::read_to_string(path).unwrap(), "sync = 42\n");
}

#[test]
fn external_change_during_edit_is_not_overwritten() {
    let dir = TempDir::new().unwrap();
    let path = write_file(&dir, "value = 1\n");
    let error = write::edit_file(&path, |doc| {
        write::set(doc, &["value"], 2)?;
        fs::write(&path, "value = 3\n").unwrap();
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(
        error,
        write::ConfigWriteError::ConcurrentEdit { .. }
    ));
    assert_eq!(fs::read_to_string(path).unwrap(), "value = 3\n");
}

#[test]
fn failed_mutation_does_not_commit_earlier_mutations() {
    let dir = TempDir::new().unwrap();
    let path = write_file(&dir, "sync = false\n");
    assert!(
        write::edit_file(&path, |doc| {
            write::set(doc, &["gateway_url"], "https://example.com")?;
            write::set(doc, &["sync", "key"], "key")
        })
        .is_err()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "sync = false\n");
}
