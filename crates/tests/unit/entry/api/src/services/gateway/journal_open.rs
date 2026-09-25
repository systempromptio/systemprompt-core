// The accounting journal is opened once at the gateway composition root; a
// missing or malformed encryption key must be a named boot failure there,
// never a per-request admission error.

use systemprompt_api::services::gateway::audit::journal::GatewayJournal;
use systemprompt_models::Secrets;

fn secrets_with_key(key: &str) -> Secrets {
    Secrets::parse(&format!(
        r#"{{
            "oauth_at_rest_pepper": "0123456789abcdef0123456789abcdef",
            "database_url": "postgres://user:pass@localhost:5432/db",
            "encryption_master_key": "{key}"
        }}"#
    ))
    .expect("secrets parse")
}

fn secrets_without_key() -> Secrets {
    Secrets::parse(
        r#"{
            "oauth_at_rest_pepper": "0123456789abcdef0123456789abcdef",
            "database_url": "postgres://user:pass@localhost:5432/db"
        }"#,
    )
    .expect("secrets parse")
}

#[test]
fn a_missing_key_names_the_secret_and_the_env_source_rule() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = GatewayJournal::open(dir.path(), &secrets_without_key()).expect_err("no key");
    let message = error.to_string();
    assert!(message.contains("encryption_master_key"), "{message}");
    assert!(message.contains("SYSTEMPROMPT_CUSTOM_SECRETS"), "{message}");
}

#[test]
fn a_key_of_the_wrong_length_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = GatewayJournal::open(dir.path(), &secrets_with_key("abcd")).expect_err("short key");
    assert!(error.to_string().contains("32-byte"), "{error}");
}

#[test]
fn a_non_hex_key_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = GatewayJournal::open(dir.path(), &secrets_with_key(&"zz".repeat(32)))
        .expect_err("non-hex key");
    assert!(error.to_string().contains("hex decode failed"), "{error}");
}

#[test]
fn a_valid_key_creates_the_journal_directory_inside_the_state_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    GatewayJournal::open(dir.path(), &secrets_with_key(&"ab".repeat(32))).expect("valid key");
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
