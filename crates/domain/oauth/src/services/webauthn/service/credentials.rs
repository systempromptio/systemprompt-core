//! `WebAuthn` credential persistence helpers.
//!
//! Stored credentials keep their transport hints as lowercase strings in the
//! `transports` column, while the serialized `webauthn_rs::prelude::Passkey`
//! blob is treated as opaque. `webauthn_rs` deserializes
//! `AuthenticatorTransport` case-sensitively, so on read the stored lowercase
//! values must be re-cased via [`normalize_transport_casing`] before the blob
//! is handed back to `webauthn_rs`. Changing this casing scheme breaks every
//! previously stored passkey.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::WebAuthnService;
use crate::error::OauthResult as Result;
use crate::repository::WebAuthnCredentialParams;
use systemprompt_identifiers::UserId;
use uuid::Uuid;
use webauthn_rs::prelude::*;

fn extract_stored_transports(passkey_json: &serde_json::Value) -> Vec<String> {
    // JSON: opaque webauthn_rs Passkey serialization — a typed struct here would
    // break stored credentials.
    passkey_json
        .get("cred")
        .and_then(|cred| cred.get("transports"))
        .and_then(|t| t.as_array())
        .map_or_else(
            || vec!["internal".to_owned()],
            |arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_lowercase))
                    .collect()
            },
        )
}

/// Re-injects stored lowercase transport hints into an opaque serialized
/// `Passkey` value.
///
/// Values are re-cased to the exact variant casing that `webauthn_rs`'s
/// case-sensitive `AuthenticatorTransport` deserialization expects
/// (`internal` → `Internal`, `usb` → `Usb`, `nfc` → `Nfc`, `ble` → `Ble`,
/// `hybrid` → `Hybrid`; anything else passes through lowercased).
///
/// This casing map is load-bearing: it compensates for the mismatch between
/// the lowercase transports persisted alongside the credential and the casing
/// `webauthn_rs` accepts, and altering it invalidates stored passkeys.
pub fn normalize_transport_casing(
    passkey_json: &mut serde_json::Value,
    stored_transports: &[String],
) {
    if let Some(credential) = passkey_json.get_mut("cred") {
        let transports_json: Vec<String> = stored_transports
            .iter()
            .map(|t| {
                t.to_lowercase()
                    .replace("internal", "Internal")
                    .replace("usb", "Usb")
                    .replace("nfc", "Nfc")
                    .replace("ble", "Ble")
                    .replace("hybrid", "Hybrid")
            })
            .collect();

        credential["transports"] = serde_json::json!(transports_json);
    }
}

impl WebAuthnService {
    pub(super) async fn store_credential(
        &self,
        user_id: &UserId,
        sk: &Passkey,
        display_name: &str,
    ) -> Result<()> {
        let credential_id = sk.cred_id().clone();
        let public_key = serde_json::to_vec(sk)?;
        let counter = 0u32;
        let id = Uuid::new_v4().to_string();

        let transports = extract_stored_transports(&serde_json::to_value(sk)?);

        let params =
            WebAuthnCredentialParams::builder(&id, user_id, &credential_id, &public_key, counter)
                .with_display_name(display_name)
                .with_device_type("platform")
                .with_transports(&transports)
                .build();

        self.oauth_repo.store_webauthn_credential(params).await
    }

    pub(super) async fn get_user_credentials(&self, user_id: &UserId) -> Result<Vec<Passkey>> {
        let credentials = self.oauth_repo.list_webauthn_credentials(user_id).await?;

        let mut passkeys = Vec::new();
        for cred in credentials {
            let mut passkey_json: serde_json::Value = serde_json::from_slice(&cred.public_key)?;

            normalize_transport_casing(&mut passkey_json, &cred.transports);

            let passkey: Passkey = serde_json::from_value(passkey_json)?;
            passkeys.push(passkey);
        }

        Ok(passkeys)
    }

    pub(super) async fn get_user_credentials_by_email(&self, email: &str) -> Result<Vec<Passkey>> {
        if let Some(user) = self.oauth_repo.find_user_by_email(email).await? {
            self.get_user_credentials(&user.id).await
        } else {
            Ok(Vec::new())
        }
    }

    pub(super) async fn update_credential_counter(
        &self,
        credential_id: &[u8],
        counter: u32,
    ) -> Result<()> {
        self.oauth_repo
            .update_webauthn_credential_counter(credential_id, counter)
            .await
    }
}
