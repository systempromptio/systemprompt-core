//! Tests for the generation diff that decides whether a reload swaps.

use systemprompt_config::{PINNED_SECRET_KEYS, SecretsBootstrapError, diff_secrets};
use systemprompt_manifest::secrets::Secrets;

const PEPPER: &str = "pppppppppppppppppppppppppppppppppppppppp";

fn secrets(extra: &str) -> Secrets {
    Secrets::parse(&format!(
        r#"{{"oauth_at_rest_pepper":"{PEPPER}","database_url":"postgres://a","manifest_signing_secret_seed":"seed","encryption_master_key":"mk"{extra}}}"#
    ))
    .expect("fixture parses")
}

#[test]
fn identical_generations_have_no_changed_keys() {
    let old = secrets(r#","anthropic":"k1""#);
    let new = secrets(r#","anthropic":"k1""#);
    assert!(diff_secrets(&old, &new).expect("diff").is_empty());
}

#[test]
fn changed_added_and_removed_keys_are_reported_sorted() {
    let old = secrets(r#","anthropic":"k1","custom_a":"x""#);
    let new = secrets(r#","anthropic":"k2","custom_b":"y""#);
    assert_eq!(
        diff_secrets(&old, &new).expect("diff"),
        vec!["anthropic", "custom_a", "custom_b"]
    );
}

#[test]
fn a_changed_pinned_key_rejects_the_reload() {
    for key in PINNED_SECRET_KEYS {
        let old = secrets("");
        let replacement = if *key == "oauth_at_rest_pepper" {
            "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq"
        } else {
            "changed"
        };
        let mut fields: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::to_value(&old).expect("encode")).expect("object");
        fields.insert(
            (*key).to_owned(),
            serde_json::Value::String(replacement.to_owned()),
        );
        let new = Secrets::parse(&serde_json::Value::Object(fields).to_string()).expect("parses");
        let err = diff_secrets(&old, &new).expect_err("pinned key change accepted");
        assert!(
            matches!(&err, SecretsBootstrapError::ReloadRejected { key: rejected } if rejected == key),
            "{key}: {err}"
        );
    }
}

#[test]
fn a_rotated_signing_key_rejects_the_reload() {
    let old = secrets(r#","signing_key_pem":"old-pem""#);
    let new = secrets(r#","signing_key_pem":"new-pem""#);
    let err = diff_secrets(&old, &new).expect_err("signing key rotation accepted");
    assert!(
        matches!(&err, SecretsBootstrapError::ReloadRejected { key } if key == "signing_key_pem"),
        "{err}"
    );
}

#[test]
fn the_pinned_set_covers_identity_and_storage_keys() {
    for key in [
        "encryption_master_key",
        "oauth_at_rest_pepper",
        "manifest_signing_secret_seed",
        "database_url",
        "database_write_url",
        "database_session_url",
        "signing_key_pem",
        "jwt_secret",
    ] {
        assert!(PINNED_SECRET_KEYS.contains(&key), "{key} is not pinned");
    }
}
