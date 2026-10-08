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
use crate::error::{OauthError, OauthResult as Result};
use crate::repository::WebAuthnCredentialParams;
use systemprompt_identifiers::UserId;
use uuid::Uuid;
use webauthn_rs::prelude::*;

// JSON: webauthn-rs `Passkey` serialisation — opaque; stored rows depend on it.
fn extract_stored_transports(passkey_json: &serde_json::Value) -> Vec<String> {
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

// JSON: webauthn-rs `Passkey` serialisation — opaque; stored rows depend on it.
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
        let id = Uuid::new_v4().to_string();

        let transports = extract_stored_transports(&serde_json::to_value(sk)?);

        let params = WebAuthnCredentialParams::builder(&id, user_id, &credential_id, &public_key)
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
            passkeys.push(decode_passkey(&cred.public_key, &cred.transports)?);
        }

        Ok(passkeys)
    }

    pub(super) async fn get_user_credentials_by_email(&self, email: &str) -> Result<Vec<Passkey>> {
        let user = self
            .user_provider
            .find_by_email(email)
            .await
            .map_err(|source| OauthError::UserProvider {
                context: "looking up the account's passkeys",
                source,
            })?;
        if let Some(user) = user {
            self.get_user_credentials(&user.id).await
        } else {
            Ok(Vec::new())
        }
    }

    pub(super) async fn record_authentication(
        &self,
        user_id: &UserId,
        auth_result: &AuthenticationResult,
    ) -> Result<()> {
        let authenticated_id: &[u8] = auth_result.cred_id().as_ref();
        let credentials = self.oauth_repo.list_webauthn_credentials(user_id).await?;
        let stored = credentials
            .iter()
            .find(|cred| cred.credential_id.as_slice() == authenticated_id)
            .ok_or_else(|| {
                OauthError::WebAuthnVerificationFailed(
                    "authenticated credential is not registered to the user".to_owned(),
                )
            })?;

        let mut passkey = decode_passkey(&stored.public_key, &stored.transports)?;
        let changed = passkey.update_credential(auth_result).ok_or_else(|| {
            OauthError::WebAuthnVerificationFailed(
                "authenticated credential does not match the stored passkey".to_owned(),
            )
        })?;

        if auth_result.counter() > 0 && !changed {
            return Err(OauthError::WebAuthnVerificationFailed(
                "signature counter did not advance; possible cloned authenticator".to_owned(),
            ));
        }

        if changed {
            let updated = serde_json::to_vec(&passkey)?;
            self.oauth_repo
                .replace_webauthn_passkey(&stored.credential_id, &stored.public_key, &updated)
                .await
        } else {
            self.oauth_repo
                .touch_webauthn_credential(&stored.credential_id)
                .await
        }
    }
}

fn decode_passkey(blob: &[u8], stored_transports: &[String]) -> Result<Passkey> {
    let mut passkey_json: serde_json::Value = serde_json::from_slice(blob)?;
    normalize_transport_casing(&mut passkey_json, stored_transports);
    Ok(serde_json::from_value(passkey_json)?)
}
