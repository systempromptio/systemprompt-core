//! A live secrets reload never leaves a replica in a mixed signing-key state:
//! a generation that changes the active, staged or previous keys is refused
//! as a whole, and every key keeps its booted value until a restart.

use base64::Engine;
use systemprompt_config::{ConfigError, LoadedProfile, SecretsBootstrapError, SecretsService};

use crate::fixture;

const FILE_REFRESH_SECRETS: &str =
    "secrets:\n  secrets_path: secrets.json\n  source: file\n  refresh_secs: 60\n";

fn body(tag: &str) -> String {
    let b64 = |v: &str| base64::engine::general_purpose::STANDARD.encode(v);
    let previous = serde_json::to_string(&format!(
        r#"[{{"kid":"{tag}-old","public_pem":"pem","expires_at":"2030-01-01T00:00:00Z"}}]"#
    ))
    .expect("encode");
    let base = fixture::secrets_json(Some(fixture::SEED));
    base.replacen(
        '{',
        &format!(
            "{{\n  \"anthropic\": \"{tag}-provider\",\n  \"signing_key_pem\": \"{}\",\n  \
             \"signing_key_staged_pem\": \"{}\",\n  \"signing_key_previous_keys\": {previous},",
            b64(&format!("{tag}-active")),
            b64(&format!("{tag}-staged")),
        ),
        1,
    )
}

#[tokio::test]
async fn a_reload_that_changes_the_key_set_is_refused_and_keeps_every_key() {
    let fx = fixture::write_tree(FILE_REFRESH_SECRETS, Some(&body("one")));
    let profile = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let service = SecretsService::load(profile).await.expect("secrets");
    let generation = service.generation();
    let before = service.get();

    std::fs::write(&fx.secrets_path, body("two")).expect("rotate");
    let err = service
        .reload()
        .await
        .expect_err("key-set change swapped in");
    assert!(
        matches!(
            &err,
            ConfigError::Secrets(SecretsBootstrapError::ReloadRejected { .. })
        ),
        "{err}"
    );

    let after = service.get();
    assert_eq!(service.generation(), generation);
    assert_eq!(after.signing_key_pem, before.signing_key_pem);
    assert_eq!(after.signing_key_staged_pem, before.signing_key_staged_pem);
    assert_eq!(
        after.signing_key_previous_keys,
        before.signing_key_previous_keys
    );
    assert_eq!(after.anthropic.as_deref(), Some("one-provider"));
    assert_eq!(
        service.signing_key_staged_pem().expect("staged").as_deref(),
        Some("one-staged")
    );
}

#[tokio::test]
async fn a_reload_that_only_drops_the_previous_keys_is_refused() {
    let fx = fixture::write_tree(FILE_REFRESH_SECRETS, Some(&body("one")));
    let profile = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let service = SecretsService::load(profile).await.expect("secrets");

    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fx.secrets_path).expect("read"))
            .expect("json");
    doc.as_object_mut()
        .expect("object")
        .remove("signing_key_previous_keys");
    std::fs::write(&fx.secrets_path, doc.to_string()).expect("write");

    service
        .reload()
        .await
        .expect_err("previous-key removal swapped in");
    assert_eq!(service.signing_keys_previous().expect("previous").len(), 1);
}
