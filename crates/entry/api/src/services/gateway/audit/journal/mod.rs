//! Encrypted terminal receipts retained until database settlement succeeds.
//!
//! [`GatewayJournal`] is built once at the gateway composition root from the
//! storage data directory and the `encryption_master_key` secret, and injected
//! into every audit. It lives under `paths.storage`, never beside the profile:
//! the profile is configuration and is mounted read-only in the self-host
//! bundle, and receipts belong to the node that admitted them, so the data
//! directory must be a per-node writable volume. Admission only reserves a receipt; settlement of
//! receipts left behind by a crash runs from [`spawn_recovery`], an owned
//! task, never on the request path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod files;
mod settle;
mod types;

pub(super) use types::{CapturedToolCall, Completion, PartialUsage, Receipt};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::aead::KeyInit;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_models::Secrets;
use tokio::task::JoinHandle;

pub const RECOVERY_INTERVAL: Duration = Duration::from_secs(30);
pub use systemprompt_ai::repository::ai_requests::ORPHAN_AGE;

#[derive(Clone)]
pub struct GatewayJournal {
    root: PathBuf,
    cipher: ChaCha20Poly1305,
}

impl std::fmt::Debug for GatewayJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayJournal")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl GatewayJournal {
    pub fn open(state_dir: &Path, secrets: &Secrets) -> Result<Self> {
        let key = secrets.get("encryption_master_key").context(
            "Gateway accounting journal requires the `encryption_master_key` secret (32 bytes \
             as 64 hex characters); with `secrets.source: env` it must also be listed in \
             SYSTEMPROMPT_CUSTOM_SECRETS",
        )?;
        let decoded = hex::decode(key).context("encryption_master_key is not hex")?;
        let cipher = ChaCha20Poly1305::new_from_slice(&decoded).map_err(|error| {
            anyhow::anyhow!("encryption_master_key is not a 32-byte key: {error}")
        })?;
        let root = state_dir.join("gateway-journal");
        std::fs::create_dir_all(&root)
            .with_context(|| format!("Cannot create gateway journal at {}", root.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self { root, cipher })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    const fn cipher(&self) -> &ChaCha20Poly1305 {
        &self.cipher
    }
}

#[derive(Clone)]
pub struct Settlement {
    pub journal: Arc<GatewayJournal>,
    pub requests: Arc<AiRequestRepository>,
    /// Absent only where the analytics layer is unavailable; settlement then
    /// records the request and skips the session counters.
    pub sessions: Option<systemprompt_traits::DynSessionStore>,
}

impl std::fmt::Debug for Settlement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settlement")
            .field("journal", &self.journal)
            .finish_non_exhaustive()
    }
}

pub(super) async fn reserve(
    journal: &Arc<GatewayJournal>,
    receipt: Receipt,
) -> Result<std::fs::File> {
    let journal = Arc::clone(journal);
    tokio::task::spawn_blocking(move || files::reserve(&journal, &receipt)).await?
}

pub(super) async fn record(settlement: &Settlement, receipt: Receipt) -> Result<()> {
    let bytes = serde_json::to_vec(&receipt)?;
    let id = receipt.request_id.clone();
    let journal = Arc::clone(&settlement.journal);
    tokio::task::spawn_blocking(move || files::replace(&journal, &id, &bytes)).await??;
    settle::settle(settlement, &receipt).await?;
    let id = receipt.request_id;
    let journal = Arc::clone(&settlement.journal);
    tokio::task::spawn_blocking(move || files::remove(&journal, &id)).await??;
    Ok(())
}

pub(super) async fn record_accounting_failure(
    settlement: &Settlement,
    receipt: Receipt,
) -> Result<()> {
    let journal = Arc::clone(&settlement.journal);
    let bytes = serde_json::to_vec(&receipt)?;
    tokio::task::spawn_blocking(move || {
        let receipt: Receipt = serde_json::from_slice(&bytes)?;
        files::append_accounting_failure(&journal, &receipt)
    })
    .await??;
    settle::settle(settlement, &receipt).await?;
    let id = receipt.storage_id();
    let journal = Arc::clone(&settlement.journal);
    tokio::task::spawn_blocking(move || files::remove(&journal, &id)).await??;
    Ok(())
}

pub(super) async fn settle_unadmitted_failure(
    settlement: &Settlement,
    receipt: &Receipt,
) -> Result<()> {
    anyhow::ensure!(
        receipt.completion.is_none() && receipt.failure.is_some(),
        "Only an unadmitted failure may bypass the terminal journal"
    );
    settle::settle(settlement, receipt).await
}

pub async fn recover(settlement: &Settlement) -> Result<usize> {
    let journal = Arc::clone(&settlement.journal);
    let listed = tokio::task::spawn_blocking(move || files::list(&journal)).await??;
    let mut settled = 0;
    for entry in listed {
        let receipt = match entry {
            files::Listed::Receipt(receipt) => *receipt,
            files::Listed::Quarantined { path, reason } => {
                tracing::error!(path = %path.display(), %reason, "Gateway journal receipt unreadable; quarantined");
                let journal = Arc::clone(&settlement.journal);
                let target = path.clone();
                if let Err(error) =
                    tokio::task::spawn_blocking(move || files::quarantine(&journal, &target))
                        .await?
                {
                    tracing::error!(path = %path.display(), %error, "Gateway journal quarantine failed");
                }
                continue;
            },
        };
        if receipt.completion.is_none()
            && receipt.failure.is_none()
            && receipt.accounting_failure.is_none()
        {
            continue;
        }
        let id = receipt.storage_id();
        if let Err(error) = settle::settle(settlement, &receipt).await {
            tracing::warn!(ai_request_id = %id, %error, "Gateway accounting recovery pending; receipt retained");
            continue;
        }
        let journal = Arc::clone(&settlement.journal);
        tokio::task::spawn_blocking(move || files::remove(&journal, &id)).await??;
        settled += 1;
    }
    for orphan in settlement
        .requests
        .fail_orphaned_pending(ORPHAN_AGE)
        .await?
    {
        tracing::warn!(
            ai_request_id = %orphan.id,
            user_id = %orphan.owner,
            "Gateway request outlived every receipt; failed with unknown usage"
        );
    }
    Ok(settled)
}

pub fn spawn_recovery(settlement: Settlement) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(RECOVERY_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match recover(&settlement).await {
                Ok(0) => {},
                Ok(settled) => tracing::info!(settled, "Gateway accounting receipts recovered"),
                Err(error) => tracing::error!(%error, "Gateway accounting recovery failed"),
            }
        }
    })
}
