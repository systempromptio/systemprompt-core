//! Where an account's outbox lives and how it is read and written: owner-only
//! JSON under the state dir, named by a hash of the account id. An outbox of
//! another device, or one that does not decode, reads as empty: it only holds
//! evidence the next capture produces again. Every call blocks on the
//! filesystem.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use sp_protocol::ids::{AccountId, DeviceId};
use sp_protocol::names::sha256_hex;
use systemprompt_device::secure::{SecureError, read_optional, write_private};

use super::Outbox;

const OUTBOX_DIR: &str = "outbox";
const OUTBOX_BYTES_MAX: usize = 16 * 1024 * 1024;

/// Why the outbox could not be written.
#[derive(Debug, thiserror::Error)]
pub enum OutboxError {
    #[error("the outbox could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("the outbox is larger than it may grow")]
    TooLarge,
    #[error("the outbox could not be written")]
    Write(#[source] SecureError),
}

pub fn file_name(account: &AccountId) -> String {
    format!("{}.json", sha256_hex(account.as_str().as_bytes()))
}

pub fn outbox_path(state_dir: &Path, name: &str) -> PathBuf {
    state_dir.join(OUTBOX_DIR).join(name)
}

pub fn load(path: &Path, device: &DeviceId) -> Outbox {
    let bytes = match read_optional(path) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Outbox::new(device.clone()),
        Err(error) => {
            tracing::warn!(path = %path.display(), error = %error, "the outbox could not be read; starting empty");
            return Outbox::new(device.clone());
        },
    };
    if bytes.len() > OUTBOX_BYTES_MAX {
        tracing::warn!(path = %path.display(), "the outbox is too large; starting empty");
        return Outbox::new(device.clone());
    }
    match serde_json::from_slice::<Outbox>(&bytes) {
        Ok(outbox) if &outbox.device == device => outbox,
        Ok(_) => Outbox::new(device.clone()),
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                line = error.line(),
                column = error.column(),
                "the outbox does not decode; starting empty"
            );
            Outbox::new(device.clone())
        },
    }
}

pub fn save(path: &Path, outbox: &Outbox) -> Result<(), OutboxError> {
    let bytes = serde_json::to_vec(outbox).map_err(OutboxError::Encode)?;
    if bytes.len() > OUTBOX_BYTES_MAX {
        return Err(OutboxError::TooLarge);
    }
    write_private(path, &bytes).map_err(OutboxError::Write)
}
