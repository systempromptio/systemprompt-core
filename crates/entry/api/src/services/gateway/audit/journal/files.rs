//! Bounded, atomic journal writes with profile-scoped authenticated encryption.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Receipt;
use anyhow::{Context, Result, ensure};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::AiRequestId;

const MAX_ENTRIES: usize = 64;
const MAX_BYTES: usize = 16 * 1024 * 1024;

fn directory() -> Result<PathBuf> {
    let profile = systemprompt_config::ProfileBootstrap::get_path()?;
    let root = Path::new(profile)
        .parent()
        .context("Profile has no directory")?
        .join("gateway-journal");
    fs::create_dir_all(&root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    }
    Ok(root)
}

fn cipher() -> Result<ChaCha20Poly1305> {
    let secrets = systemprompt_config::SecretsBootstrap::get()?;
    let key = secrets
        .get("encryption_master_key")
        .context("Gateway journal requires encryption_master_key")?;
    let decoded = hex::decode(key)?;
    ChaCha20Poly1305::new_from_slice(&decoded)
        .map_err(|_| anyhow::anyhow!("Invalid journal encryption key"))
}

fn name(id: &AiRequestId) -> String {
    format!(
        "{}.receipt",
        hex::encode(Sha256::digest(id.as_str().as_bytes()))
    )
}

fn lock(root: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".lock"))?;
    file.lock()?;
    Ok(file)
}

pub(super) fn reserve(receipt: &Receipt) -> Result<File> {
    let root = directory()?;
    let _lock = lock(&root)?;
    let entries = fs::read_dir(&root)?.collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        entries
            .iter()
            .filter(|e| e.path().extension().is_some_and(|x| x == "receipt"))
            .count()
            < MAX_ENTRIES,
        "Gateway journal capacity exhausted"
    );
    ensure!(
        !root.join(name(&receipt.request_id)).exists(),
        "Request already admitted"
    );
    let lease = lease(&root, &receipt.request_id)?;
    lease.try_lock()?;
    write(&root, &receipt.request_id, &serde_json::to_vec(receipt)?)?;
    Ok(lease)
}

pub(super) fn replace(id: &AiRequestId, bytes: &[u8]) -> Result<()> {
    let root = directory()?;
    let _lock = lock(&root)?;
    ensure!(
        root.join(name(id)).exists(),
        "Missing durable admission receipt"
    );
    let previous = read(&root.join(name(id)), &cipher()?)?;
    ensure!(
        previous.completion.is_none() && previous.failure.is_none(),
        "Terminal accounting receipt already exists; recovery will settle it"
    );
    write(&root, id, bytes)
}

fn write(root: &Path, id: &AiRequestId, bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Gateway completion exceeds journal capacity"
    );
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher()?
        .encrypt(&Nonce::from(nonce), bytes)
        .map_err(|_| anyhow::anyhow!("Journal encryption failed"))?;
    let temp = root.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(&nonce)?;
    file.write_all(&encrypted)?;
    file.sync_all()?;
    fs::rename(&temp, root.join(name(id)))?;
    File::open(&root)?.sync_all()?;
    Ok(())
}

pub(super) fn remove(id: &AiRequestId) -> Result<()> {
    let root = directory()?;
    let _lock = lock(&root)?;
    match fs::remove_file(root.join(name(id))) {
        Ok(()) => File::open(&root)?.sync_all()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(e.into()),
    }
    match fs::remove_file(root.join(format!("{}.lease", name(id)))) {
        Ok(()) => File::open(&root)?.sync_all()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

pub(super) fn list() -> Result<Vec<Receipt>> {
    let root = directory()?;
    let _lock = lock(&root)?;
    let cipher = cipher()?;
    let mut result = Vec::new();
    for entry in fs::read_dir(&root)? {
        let path = entry?.path();
        // Why: an interrupted atomic write must not accumulate unbounded temporary
        // files.
        if path.extension().is_some_and(|x| x == "tmp") {
            fs::remove_file(&path)?;
            continue;
        }
        if !path.extension().is_some_and(|x| x == "receipt") {
            continue;
        }
        let mut receipt = read(&path, &cipher)?;
        if receipt.completion.is_none() && receipt.failure.is_none() {
            let lease = lease(&root, &receipt.request_id)?;
            match lease.try_lock() {
                Ok(()) => {
                    receipt.failure = Some("Accounting incomplete: request ended before a terminal receipt was persisted; upstream usage is unknown".to_owned());
                    write(&root, &receipt.request_id, &serde_json::to_vec(&receipt)?)?;
                },
                Err(fs::TryLockError::WouldBlock) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        result.push(receipt);
    }
    Ok(result)
}

fn read(path: &Path, cipher: &ChaCha20Poly1305) -> Result<Receipt> {
    ensure!(
        fs::metadata(path)?.len() <= (MAX_BYTES + 64) as u64,
        "Oversized journal receipt"
    );
    let bytes = fs::read(path)?;
    ensure!(bytes.len() >= 12, "Truncated journal receipt");
    let nonce: [u8; 12] = bytes[..12].try_into()?;
    let plain = cipher
        .decrypt(&Nonce::from(nonce), &bytes[12..])
        .map_err(|_| anyhow::anyhow!("Journal authentication failed"))?;
    Ok(serde_json::from_slice(&plain)?)
}

fn lease(root: &Path, id: &AiRequestId) -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(format!("{}.lease", name(id))))?)
}
