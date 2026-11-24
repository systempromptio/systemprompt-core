//! Encrypted terminal receipts retained until database settlement succeeds.
//!
//! [`GatewayJournal`] is built once at the gateway composition root from the
//! storage data directory and the decoded `encryption_master_key`, and injected
//! into every audit. It lives under `paths.storage`, never beside the profile:
//! the profile is configuration and is mounted read-only in the self-host
//! bundle, and receipts belong to the node that admitted them, so the data
//! directory must be a per-node writable volume.
//!
//! The journal is an append-only log of encrypted frames in segments the
//! process owns, indexed in memory. Writes are group-committed: concurrent
//! writers share one append and one sync, and none is acknowledged before the
//! sync that covers it returns. Admission only reserves a receipt, counted
//! against the profile's `governance.quota.admission_limit`; past the limit an
//! admission is refused as [`GatewayAuditError::AdmissionLimit`], which the
//! gateway answers with 429. Settlement of receipts left behind by a crash
//! runs from [`spawn_recovery`], a cancellable task on the server's
//! `BackgroundTasks`, never on the request path. On cancellation — the
//! server's shutdown drain — the task runs one final settlement pass before
//! it returns, so a node that terminates in order leaves its journal empty;
//! the drain's background grace bounds that pass.
//!
//! Every pass publishes `gateway_journal_receipts` (receipts still held) and
//! counts `gateway_journal_settle_failures_total` (receipts retained because
//! settlement failed), `gateway_journal_recovery_failures_total` (passes that
//! failed outright) and `gateway_journal_torn_frames_total` (segments of a
//! dead process that ended in a torn frame). Every flush counts
//! `gateway_journal_fsyncs_total` and `gateway_journal_records_total`; a
//! refused admission counts `gateway_journal_admissions_shed_total`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod index;
mod log;
mod ops;
mod recovery;
mod segment;
mod settle;
mod types;

pub use log::{Fsync, JOURNAL_FSYNCS_TOTAL, JOURNAL_RECORDS_TOTAL, JournalSync};
pub(crate) use ops::JournalLease;
pub(super) use types::{CapturedToolCall, Completion, PartialUsage, Receipt};

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::error::{GatewayAuditError, GatewayAuditResult as Result};
use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::aead::KeyInit;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_traits::BackgroundTasks;

pub const RECOVERY_INTERVAL: Duration = Duration::from_secs(30);
pub const JOURNAL_RECEIPTS: &str = "gateway_journal_receipts";
pub const JOURNAL_SETTLE_FAILURES_TOTAL: &str = "gateway_journal_settle_failures_total";
pub const JOURNAL_RECOVERY_FAILURES_TOTAL: &str = "gateway_journal_recovery_failures_total";
pub const JOURNAL_TORN_FRAMES_TOTAL: &str = "gateway_journal_torn_frames_total";
pub const JOURNAL_ADMISSIONS_SHED_TOTAL: &str = "gateway_journal_admissions_shed_total";
pub use systemprompt_ai::repository::ai_requests::ORPHAN_AGE;

#[derive(Clone)]
pub struct GatewayJournal {
    log: Arc<log::Log>,
    recovering: Arc<tokio::sync::Mutex<()>>,
}

impl std::fmt::Debug for GatewayJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayJournal")
            .field("root", &self.log.root)
            .finish_non_exhaustive()
    }
}

impl GatewayJournal {
    pub fn open(state_dir: &Path, key: [u8; 32]) -> Result<Self> {
        Self::open_with(state_dir, key, Arc::new(Fsync))
    }

    pub fn open_with(state_dir: &Path, key: [u8; 32], sync: Arc<dyn JournalSync>) -> Result<Self> {
        let cipher = ChaCha20Poly1305::new(&key.into());
        let root: PathBuf = state_dir.join("gateway-journal");
        systemprompt_config::create_state_dir(&root).map_err(GatewayAuditError::JournalDir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        let legacy = recovery::legacy_receipts(&root)?;
        if legacy > 0 {
            tracing::warn!(
                root = %root.display(),
                legacy,
                "Gateway journal holds per-file receipts from an earlier release; they are not replayed"
            );
        }
        Ok(Self {
            log: Arc::new(log::Log::create(root, cipher, sync)?),
            recovering: Arc::new(tokio::sync::Mutex::new(())),
        })
    }
}

/// The collaborators a terminal receipt is settled through.
///
/// `sessions` is absent only where no session usage counters were injected;
/// settlement then records the request and skips the session counters.
#[derive(Clone)]
pub struct Settlement {
    pub journal: Arc<GatewayJournal>,
    pub requests: Arc<AiRequestRepository>,
    pub sessions: Option<systemprompt_traits::DynSessionUsageCounters>,
}

impl std::fmt::Debug for Settlement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settlement")
            .field("journal", &self.journal)
            .finish_non_exhaustive()
    }
}

async fn blocking<T: Send + 'static>(
    journal: &GatewayJournal,
    work: impl FnOnce(&Arc<log::Log>) -> Result<T> + Send + 'static,
) -> Result<T> {
    let log = Arc::clone(&journal.log);
    tokio::task::spawn_blocking(move || work(&log)).await?
}

pub(super) async fn reserve(
    journal: &GatewayJournal,
    receipt: Receipt,
    limit: NonZeroU32,
) -> Result<JournalLease> {
    blocking(journal, move |log| ops::reserve(log, &receipt, limit)).await
}

pub(super) async fn record(settlement: &Settlement, receipt: Receipt) -> Result<()> {
    let receipt = Arc::new(receipt);
    let held = Arc::clone(&receipt);
    blocking(&settlement.journal, move |log| {
        ops::put_terminal(log, &held)
    })
    .await?;
    settle::settle(settlement, &receipt).await?;
    let id = receipt.request_id.clone();
    blocking(&settlement.journal, move |log| ops::remove(log, &id)).await
}

pub(super) async fn record_accounting_failure(
    settlement: &Settlement,
    receipt: Receipt,
) -> Result<()> {
    let receipt = Arc::new(receipt);
    let held = Arc::clone(&receipt);
    blocking(&settlement.journal, move |log| {
        ops::put_accounting_failure(log, &held)
    })
    .await?;
    settle::settle(settlement, &receipt).await?;
    let id = receipt.storage_id();
    blocking(&settlement.journal, move |log| ops::remove(log, &id)).await
}

pub(super) async fn settle_unadmitted_failure(
    settlement: &Settlement,
    receipt: &Receipt,
) -> Result<()> {
    crate::error::ensure(
        receipt.completion.is_none() && receipt.failure.is_some(),
        "Only an unadmitted failure may bypass the terminal journal",
    )?;
    settle::settle(settlement, receipt).await
}

pub fn pending(journal: &GatewayJournal) -> Result<usize> {
    ops::count(&journal.log)
}

pub async fn recover(settlement: &Settlement) -> Result<usize> {
    let _pass = settlement.journal.recovering.lock().await;
    let outcome = settle_listed(settlement).await;
    match pending(&settlement.journal) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "bounded by the journal's 4096-entry capacity"
        )]
        Ok(remaining) => metrics::gauge!(JOURNAL_RECEIPTS).set(remaining as f64),
        Err(error) => tracing::warn!(%error, "Gateway journal receipts could not be counted"),
    }
    if outcome.is_err() {
        metrics::counter!(JOURNAL_RECOVERY_FAILURES_TOTAL).increment(1);
    }
    outcome
}

async fn settle_listed(settlement: &Settlement) -> Result<usize> {
    let (adoption, listed) = blocking(&settlement.journal, |log| {
        let adoption = recovery::adopt(log)?;
        Ok((adoption, recovery::settleable(log)?))
    })
    .await?;
    if adoption.torn > 0 {
        metrics::counter!(JOURNAL_TORN_FRAMES_TOTAL).increment(adoption.torn);
    }
    if adoption.adopted > 0 {
        tracing::info!(
            adopted = adoption.adopted,
            "Gateway journal adopted receipts of an exited process"
        );
    }
    let mut settled = 0;
    for receipt in listed {
        let id = receipt.storage_id();
        if let Err(error) = settle::settle(settlement, &receipt).await {
            metrics::counter!(JOURNAL_SETTLE_FAILURES_TOTAL).increment(1);
            tracing::warn!(ai_request_id = %id, %error, "Gateway accounting recovery pending; receipt retained");
            continue;
        }
        blocking(&settlement.journal, move |log| ops::remove(log, &id)).await?;
        settled += 1;
    }
    blocking(&settlement.journal, |log| recovery::compact(log)).await?;
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

pub fn spawn_recovery(settlement: Settlement, tasks: &BackgroundTasks) {
    tasks.spawn_cancellable("gateway_accounting_recovery", |cancel| async move {
        let mut tick = tokio::time::interval(RECOVERY_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = tick.tick() => {},
            }
            match recover(&settlement).await {
                Ok(0) => {},
                Ok(settled) => tracing::info!(settled, "Gateway accounting receipts recovered"),
                Err(error) => tracing::error!(%error, "Gateway accounting recovery failed"),
            }
        }
        flush_on_shutdown(&settlement).await;
    });
}

pub async fn flush_on_shutdown(settlement: &Settlement) {
    match recover(settlement).await {
        Ok(settled) => match pending(&settlement.journal) {
            Ok(0) => tracing::info!(settled, "Gateway journal flushed on shutdown"),
            Ok(remaining) => tracing::warn!(
                settled,
                remaining,
                "Gateway journal receipts remain after the shutdown flush"
            ),
            Err(error) => {
                tracing::warn!(settled, %error, "Gateway journal could not be counted after the shutdown flush");
            },
        },
        Err(error) => tracing::error!(%error, "Gateway journal shutdown flush failed"),
    }
}
