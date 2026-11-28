//! The keys a started instance generates or loads from a key file, and the
//! test tokens it signs with them: compact EdDSA JWTs for sessions and hook
//! calls.
//!
//! A key file holds the three 32-byte seeds in order (manifest, rotated,
//! tokens). It is written owner-only through the device crate's private file
//! helper the first time a path is used, and read back on every later start,
//! so a restart signs with the keys an app pinned at the first.

use std::fmt;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chrono::Utc;
use ed25519_dalek::{Signature, Signer, SigningKey};
use serde_json::{Map, Value, json};
use systemprompt_device::secure::{read_optional, write_private};

use crate::error::ScriptedInstanceError;

pub(crate) const TOKEN_TTL_SECS: i64 = 3_600;

const SEED_LEN: usize = 32;
const KEY_FILE_LEN: usize = SEED_LEN * 3;

pub(crate) const SESSION_USE: &str = "session";
pub(crate) const HOOK_USE: &str = "hook";
pub(crate) const DEVICE_TOKEN_USE: &str = "device_token";
pub(crate) const SYNC_USE: &str = "drive_sync";

const TOKEN_USE: &str = "token_use";

pub(crate) struct Keys {
    manifest: SigningKey,
    rotated: SigningKey,
    tokens: SigningKey,
    reloaded: bool,
}

impl fmt::Debug for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keys").finish_non_exhaustive()
    }
}

impl Keys {
    pub(crate) fn generate() -> Self {
        let key = || SigningKey::from_bytes(&rand::random::<[u8; 32]>());
        Self {
            manifest: key(),
            rotated: key(),
            tokens: key(),
            reloaded: false,
        }
    }

    pub(crate) fn load_or_generate(path: &Path) -> Result<Self, ScriptedInstanceError> {
        let key_file = |source| ScriptedInstanceError::KeyFile {
            path: path.to_path_buf(),
            source,
        };
        if let Some(bytes) = read_optional(path).map_err(key_file)? {
            return Self::from_seeds(&bytes).ok_or_else(|| {
                ScriptedInstanceError::KeyFileMalformed {
                    path: path.to_path_buf(),
                    expected: KEY_FILE_LEN,
                    found: bytes.len(),
                }
            });
        }
        let keys = Self::generate();
        write_private(path, &keys.seeds()).map_err(key_file)?;
        Ok(keys)
    }

    fn from_seeds(bytes: &[u8]) -> Option<Self> {
        let seeds: &[u8; KEY_FILE_LEN] = bytes.try_into().ok()?;
        let key = |index: usize| {
            let start = index * SEED_LEN;
            let seed: [u8; SEED_LEN] = seeds[start..start + SEED_LEN].try_into().ok()?;
            Some(SigningKey::from_bytes(&seed))
        };
        Some(Self {
            manifest: key(0)?,
            rotated: key(1)?,
            tokens: key(2)?,
            reloaded: true,
        })
    }

    fn seeds(&self) -> Vec<u8> {
        [&self.manifest, &self.rotated, &self.tokens]
            .iter()
            .flat_map(|key| key.to_bytes())
            .collect()
    }

    pub(crate) const fn reloaded(&self) -> bool {
        self.reloaded
    }

    pub(crate) const fn manifest_key(&self, rotated: bool) -> &SigningKey {
        if rotated {
            &self.rotated
        } else {
            &self.manifest
        }
    }

    pub(crate) fn mint(&self, token_use: &str, mut claims: Map<String, Value>) -> String {
        let now = Utc::now().timestamp();
        claims.insert(TOKEN_USE.to_owned(), json!(token_use));
        claims.insert("iat".to_owned(), json!(now));
        claims.insert("exp".to_owned(), json!(now + TOKEN_TTL_SECS));
        let header = URL_SAFE_NO_PAD.encode(json!({ "alg": "EdDSA", "typ": "JWT" }).to_string());
        let payload = URL_SAFE_NO_PAD.encode(Value::Object(claims).to_string());
        let signed = format!("{header}.{payload}");
        let signature = self.tokens.sign(signed.as_bytes());
        format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
    }

    pub(crate) fn claims(&self, token_use: &str, token: &str) -> Option<Map<String, Value>> {
        let (signed, signature) = token.rsplit_once('.')?;
        let (_header, payload) = signed.split_once('.')?;
        let signature = Signature::from_slice(&URL_SAFE_NO_PAD.decode(signature).ok()?).ok()?;
        self.tokens
            .verifying_key()
            .verify_strict(signed.as_bytes(), &signature)
            .ok()?;
        let claims: Map<String, Value> =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
        let live = claims.get("exp").and_then(Value::as_i64)? > Utc::now().timestamp();
        let used_as = claims.get(TOKEN_USE).and_then(Value::as_str)? == token_use;
        (live && used_as).then_some(claims)
    }
}

pub(crate) fn public_key(key: &SigningKey) -> String {
    STANDARD.encode(key.verifying_key().to_bytes())
}

pub(crate) fn sign(key: &SigningKey, payload: &str) -> String {
    STANDARD.encode(key.sign(payload.as_bytes()).to_bytes())
}
