//! Encrypted terminal receipts retained until database settlement succeeds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod files;
use crate::repository::gateway_accounting as settlement;
pub(super) use crate::repository::gateway_accounting::types::{Completion, Receipt};

use anyhow::Result;
use sqlx::PgPool;

pub(super) async fn reserve(receipt: Receipt, pool: &PgPool) -> Result<std::fs::File> {
    static STARTED: std::sync::Once = std::sync::Once::new();
    recover(pool).await?;
    let pool = pool.clone();
    STARTED.call_once(|| {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                tick.tick().await;
                if let Err(error) = recover(&pool).await {
                    tracing::error!(%error, "Gateway accounting recovery pending");
                }
            }
        });
    });
    tokio::task::spawn_blocking(move || files::reserve(&receipt)).await?
}

pub(super) async fn record(receipt: Receipt, pool: &PgPool) -> Result<()> {
    let bytes = serde_json::to_vec(&receipt)?;
    let id = receipt.request_id.clone();
    tokio::task::spawn_blocking(move || files::replace(&id, &bytes)).await??;
    settlement::settle(pool, &receipt).await?;
    let id = receipt.request_id;
    tokio::task::spawn_blocking(move || files::remove(&id)).await??;
    Ok(())
}

pub(crate) async fn recover(pool: &PgPool) -> Result<()> {
    let receipts = tokio::task::spawn_blocking(files::list).await??;
    for receipt in receipts {
        if receipt.completion.is_none() && receipt.failure.is_none() {
            continue;
        }
        settlement::settle(pool, &receipt).await?;
        let id = receipt.request_id;
        tokio::task::spawn_blocking(move || files::remove(&id)).await??;
    }
    Ok(())
}

pub(super) async fn settle_unadmitted_failure(pool: &PgPool, receipt: &Receipt) -> Result<()> {
    anyhow::ensure!(
        receipt.completion.is_none() && receipt.failure.is_some(),
        "Only an unadmitted failure may bypass the terminal journal"
    );
    settlement::settle(pool, receipt).await
}
