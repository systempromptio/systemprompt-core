// The accounting journal is opened once at the gateway composition root with
// the decoded `encryption_master_key`; validating that secret is the secrets
// bootstrap's job, so the journal takes key bytes and never reads the
// secrets document itself.

use systemprompt_gateway::audit::journal::GatewayJournal;

const KEY: [u8; 32] = [0xab; 32];

#[test]
fn a_valid_key_creates_the_journal_directory_inside_the_state_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    GatewayJournal::open(dir.path(), KEY).expect("valid key");
    let journal_dir = dir.path().join("gateway-journal");
    assert!(journal_dir.is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&journal_dir)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "journal directory is owner-only");
    }
}

#[test]
fn an_unwritable_state_dir_is_a_named_boot_failure() {
    let file = tempfile::NamedTempFile::new().expect("tempfile");
    let error = GatewayJournal::open(file.path(), KEY).expect_err("a file is not a state dir");
    assert!(error.to_string().contains("gateway journal"), "{error}");
}

#[test]
fn the_journal_debug_names_its_root_and_never_its_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal = GatewayJournal::open(dir.path(), KEY).expect("valid key");
    let rendered = format!("{journal:?}");
    assert!(rendered.contains("gateway-journal"), "{rendered}");
    assert!(!rendered.contains("171"), "{rendered}");
}
