//! Bounded, atomic journal writes with profile-scoped authenticated encryption.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{GatewayJournal, Receipt};
use anyhow::{Result, ensure};
use chacha20poly1305::Nonce;
use chacha20poly1305::aead::Aead;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::AiRequestId;

// Why: a receipt is held for the whole life of an in-flight request, so this is
// a disk-safety bound against a runaway directory, not a concurrency ceiling.
const MAX_ENTRIES: usize = 4096;
const MAX_BYTES: usize = 16 * 1024 * 1024;

pub(super) enum Listed {
    Receipt(Box<Receipt>),
    Quarantined { path: PathBuf, reason: String },
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

pub(super) fn reserve(journal: &GatewayJournal, receipt: &Receipt) -> Result<File> {
    let root = journal.root();
    let _lock = lock(root)?;
    let entries = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
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
    let lease = lease(root, &receipt.request_id)?;
    lease.try_lock()?;
    write(journal, &receipt.request_id, &serde_json::to_vec(receipt)?)?;
    Ok(lease)
}

pub(super) fn append_accounting_failure(journal: &GatewayJournal, receipt: &Receipt) -> Result<()> {
    let root = journal.root();
    let _lock = lock(root)?;
    let id = receipt.storage_id();
    let destination = root.join(name(&id));
    ensure!(
        receipt
            .accounting_failure
            .as_ref()
            .is_some_and(|error| !error.is_empty() && error.len() <= 4096)
            && receipt.completion.is_none()
            && receipt.failure.is_none(),
        "Invalid accounting failure receipt"
    );
    if destination.exists() {
        let previous = read(journal, &destination)?;
        ensure!(
            previous.request_id == receipt.request_id
                && previous.user_id == receipt.user_id
                && previous.accounting_failure == receipt.accounting_failure,
            "Conflicting retained accounting failure"
        );
        return Ok(());
    }
    ensure!(
        fs::read_dir(root)?
            .collect::<std::io::Result<Vec<_>>>()?
            .iter()
            .filter(|entry| entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "receipt"))
            .count()
            < MAX_ENTRIES,
        "Gateway accounting journal is full"
    );
    write(journal, &id, &serde_json::to_vec(receipt)?)
}

pub(super) fn replace(journal: &GatewayJournal, id: &AiRequestId, bytes: &[u8]) -> Result<()> {
    let root = journal.root();
    let _lock = lock(root)?;
    ensure!(
        root.join(name(id)).exists(),
        "Missing durable admission receipt"
    );
    let previous = read(journal, &root.join(name(id)))?;
    ensure!(
        previous.completion.is_none() && previous.failure.is_none(),
        "Terminal accounting receipt already exists; recovery will settle it"
    );
    write(journal, id, bytes)
}

fn write(journal: &GatewayJournal, id: &AiRequestId, bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Gateway completion exceeds journal capacity"
    );
    let root = journal.root();
    let nonce: [u8; 12] = rand::random();
    let encrypted = journal
        .cipher()
        .encrypt(&Nonce::from(nonce), bytes)
        .map_err(|error| anyhow::anyhow!("Journal encryption failed: {error}"))?;
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
    File::open(root)?.sync_all()?;
    Ok(())
}

pub(super) fn remove(journal: &GatewayJournal, id: &AiRequestId) -> Result<()> {
    let root = journal.root();
    let _lock = lock(root)?;
    for path in [
        root.join(name(id)),
        root.join(format!("{}.lease", name(id))),
    ] {
        match fs::remove_file(&path) {
            Ok(()) => File::open(root)?.sync_all()?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub(super) fn quarantine(journal: &GatewayJournal, path: &Path) -> Result<()> {
    let root = journal.root();
    let _lock = lock(root)?;
    let mut target = path.as_os_str().to_owned();
    target.push(".bad");
    fs::rename(path, &target)?;
    File::open(root)?.sync_all()?;
    Ok(())
}

pub(super) fn list(journal: &GatewayJournal) -> Result<Vec<Listed>> {
    let root = journal.root();
    let _lock = lock(root)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        // Why: an interrupted atomic write must not accumulate unbounded temporary
        // files.
        if path.extension().is_some_and(|x| x == "tmp") {
            fs::remove_file(&path)?;
            continue;
        }
        if path.extension().is_none_or(|x| x != "receipt") {
            continue;
        }
        let mut receipt = match read(journal, &path) {
            Ok(receipt) => receipt,
            Err(error) => {
                result.push(Listed::Quarantined {
                    path,
                    reason: error.to_string(),
                });
                continue;
            },
        };
        if receipt.completion.is_none()
            && receipt.failure.is_none()
            && receipt.accounting_failure.is_none()
        {
            let lease = lease(root, &receipt.request_id)?;
            match lease.try_lock() {
                Ok(()) => {
                    receipt.failure = Some("Accounting incomplete: request ended before a terminal receipt was persisted; upstream usage is unknown".to_owned());
                    write(journal, &receipt.request_id, &serde_json::to_vec(&receipt)?)?;
                },
                Err(fs::TryLockError::WouldBlock) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        result.push(Listed::Receipt(Box::new(receipt)));
    }
    Ok(result)
}

fn read(journal: &GatewayJournal, path: &Path) -> Result<Receipt> {
    ensure!(
        fs::metadata(path)?.len() <= (MAX_BYTES + 64) as u64,
        "Oversized journal receipt"
    );
    let bytes = fs::read(path)?;
    ensure!(bytes.len() >= 12, "Truncated journal receipt");
    let nonce: [u8; 12] = bytes[..12].try_into()?;
    let plain = journal
        .cipher()
        .decrypt(&Nonce::from(nonce), &bytes[12..])
        .map_err(|error| anyhow::anyhow!("Journal authentication failed: {error}"))?;
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
