//! The deploy secret set clears the rotation-managed signing keys a local
//! rotation removed, so a set-only provider push cannot leave a promoted-away
//! staged key or a revoked previous-key list trusted on the target.

use chrono::{Duration, Utc};
use systemprompt_cloud::secrets_env::{cleared_when_absent, deploy_secret_env, load_secrets_json};
use systemprompt_config::{PreviousSigningKey, SigningKeyState, persist_signing_key_state};
use tempfile::TempDir;

const PEPPER: &str = "pppppppppppppppppppppppppppppppppppppppp";

fn state(staged: Option<&str>, previous: Vec<PreviousSigningKey>) -> SigningKeyState {
    SigningKeyState {
        active_private_pem: "active-pem".to_owned(),
        staged_private_pem: staged.map(str::to_owned),
        previous,
    }
}

fn previous() -> PreviousSigningKey {
    PreviousSigningKey {
        kid: "old-kid".to_owned(),
        public_pem: "-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n".to_owned(),
        expires_at: Utc::now() + Duration::days(30),
    }
}

fn pushed_after(states: &[SigningKeyState]) -> std::collections::HashMap<String, String> {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(
        &path,
        format!(r#"{{"oauth_at_rest_pepper":"{PEPPER}","database_url":"postgres://a"}}"#),
    )
    .expect("write");
    for state in states {
        persist_signing_key_state(&path, state).expect("persist");
    }
    deploy_secret_env(load_secrets_json(&path).expect("load"))
}

#[test]
fn after_a_revoking_rotation_the_deploy_clears_the_previous_keys_and_staged_key() {
    let pushed = pushed_after(&[
        state(Some("staged-pem"), vec![previous()]),
        state(None, Vec::new()),
    ]);
    assert_eq!(
        pushed.get("SIGNING_KEY_PREVIOUS_KEYS").map(String::as_str),
        Some("")
    );
    assert_eq!(
        pushed.get("SIGNING_KEY_STAGED_PEM").map(String::as_str),
        Some("")
    );
    assert!(pushed.get("SIGNING_KEY_PEM").is_some_and(|v| !v.is_empty()));
}

#[test]
fn a_live_key_set_is_pushed_with_its_values() {
    let pushed = pushed_after(&[state(Some("staged-pem"), vec![previous()])]);
    assert!(
        pushed
            .get("SIGNING_KEY_PREVIOUS_KEYS")
            .is_some_and(|v| v.contains("old-kid"))
    );
    assert!(
        pushed
            .get("SIGNING_KEY_STAGED_PEM")
            .is_some_and(|v| !v.is_empty())
    );
}

#[test]
fn the_cleared_names_are_the_rotation_managed_secrets() {
    assert_eq!(
        cleared_when_absent(),
        [
            "SIGNING_KEY_STAGED_PEM".to_owned(),
            "SIGNING_KEY_PREVIOUS_KEYS".to_owned()
        ]
    );
}
