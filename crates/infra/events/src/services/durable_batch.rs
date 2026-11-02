//! [`DeliveryBatch`]: several claimed outbox rows held on one transaction.
//!
//! A consumer either acknowledges every row at once
//! ([`DeliveryBatch::acknowledge_all`]) or walks the rows, acknowledges each
//! one it finished ([`DeliveryBatch::acknowledge`]) and commits the round
//! once ([`DeliveryBatch::commit`]). A row left unacknowledged is released
//! unprocessed at commit, for a later claim. Dropping the batch rolls back
//! every row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::de::DeserializeOwned;
use sqlx::{PgConnection, Postgres, Transaction};
use systemprompt_identifiers::EventOutboxId;

use super::durable::ReportingFact;
use super::repository::durable::{DurableOutboxRepository, FactRow};

#[must_use]
#[derive(Debug)]
pub struct DeliveryBatch {
    tx: Transaction<'static, Postgres>,
    rows: Vec<FactRow>,
}

impl DeliveryBatch {
    pub(crate) const fn new(tx: Transaction<'static, Postgres>, rows: Vec<FactRow>) -> Self {
        Self { tx, rows }
    }

    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = &EventOutboxId> {
        self.rows.iter().map(|row| &row.id)
    }

    pub fn facts<T: DeserializeOwned>(
        &self,
    ) -> Result<Vec<(EventOutboxId, ReportingFact<T>)>, serde_json::Error> {
        self.rows
            .iter()
            .map(|row| Ok((row.id.clone(), serde_json::from_value(row.fact.clone())?)))
            .collect()
    }

    /// Each claimed row's id and undecoded fact, in claim order, so a
    /// consumer can decode them one by one and keep a bad row from failing
    /// the others.
    // JSON: versioned facts are decoded by the registered consumer.
    pub fn entries(&self) -> Vec<(EventOutboxId, serde_json::Value)> {
        self.rows
            .iter()
            .map(|row| (row.id.clone(), row.fact.clone()))
            .collect()
    }

    pub fn connection(&mut self) -> &mut PgConnection {
        &mut self.tx
    }

    /// Marks one claimed row processed inside the batch transaction; it
    /// takes effect only when the batch commits.
    pub async fn acknowledge(&mut self, id: &EventOutboxId) -> Result<(), sqlx::Error> {
        DurableOutboxRepository::mark_processed(&mut self.tx, id).await
    }

    /// Commits the round: every acknowledged row becomes processed and every
    /// other claimed row is released for a later claim.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }

    pub async fn acknowledge_all(mut self) -> Result<(), sqlx::Error> {
        DurableOutboxRepository::mark_all_processed(
            &mut self.tx,
            self.rows.iter().map(|row| &row.id),
        )
        .await?;
        self.tx.commit().await
    }

    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.tx.rollback().await
    }
}
