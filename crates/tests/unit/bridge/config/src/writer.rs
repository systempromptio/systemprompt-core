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
         [update]\nautomatic = true\n",
    );

    write::edit_file(&path, |doc| {
        write::set(doc, &["sync", "pinned_pubkey"], "new");
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
        after.contains("[update]"),
        "update section dropped: {after}"
    );
    assert!(after.contains("automatic = true"), "{after}");
}

#[test]
fn editing_preserves_comments_and_unknown_keys() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_file(
        &dir,
        "# operator note: do not remove\n\
         gateway_url = \"https://gateway.example.com\"\n\
         a_key_this_build_does_not_know = 7\n\n\
         [update]\n# keep updates manual on this fleet\nautomatic = false\n",
    );

    write::edit_file(&path, |doc| {
        write::set(doc, &["update", "automatic"], true);
    })
    .expect("write toggle");

    let after = fs::read_to_string(&path).expect("read back");
    assert!(after.contains("# operator note: do not remove"), "{after}");
    assert!(
        after.contains("# keep updates manual on this fleet"),
        "{after}"
    );
    assert!(
        after.contains("a_key_this_build_does_not_know = 7"),
        "{after}"
    );
    assert!(after.contains("automatic = true"), "{after}");
}

#[test]
fn editing_a_malformed_file_reports_rather_than_overwriting() {
    let dir = TempDir::new().expect("tempdir");
    let body = "gateway_url = \"https://gateway.example.com\n[sync\n";
    let path = write_file(&dir, body);

    let err = write::edit_file(&path, |doc| {
        write::set(doc, &["update", "automatic"], true);
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
        write::set_if_absent(doc, &["gateway_url"], "https://installer.example.com");
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
        write::set_if_absent(doc, &["gateway_url"], "https://installer.example.com");
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
        write::remove(doc, &["pat"]);
        write::set(doc, &["session", "enabled"], true);
    })
    .expect("swap credential section");

    let after = fs::read_to_string(&path).expect("read back");
    assert!(!after.contains("[pat]"), "pat section survived: {after}");
    assert!(after.contains("enabled = true"), "{after}");
    assert!(after.contains("pinned_pubkey = \"abc\""), "{after}");
}
