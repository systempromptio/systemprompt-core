//! The settlement pass over the journal: adopt receipts an exited process
//! left behind, settle every settleable receipt, compact, fail requests that
//! outlived every receipt, and publish the pass's gauges and counters. It runs
//! periodically from [`spawn_recovery`] and once more on shutdown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::BackgroundTasks;

use super::{
    JOURNAL_RECEIPTS, JOURNAL_RECOVERY_FAILURES_TOTAL, JOURNAL_SETTLE_FAILURES_TOTAL,
    JOURNAL_TORN_FRAMES_TOTAL, ORPHAN_AGE, RECOVERY_INTERVAL, Settlement, blocking, ops, pending,
    recovery, settle,
};
use crate::error::GatewayAuditResult as Result;

#[must_use]
pub fn recovery_admitted(built: &systemprompt_extension::BuiltState) -> bool {
    built.contains(&systemprompt_identifiers::ExtensionId::new("ai"))
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
