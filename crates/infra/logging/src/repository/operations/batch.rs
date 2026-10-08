//! The flush-buffer insert: one `UNNEST` statement per batch, committed with
//! `synchronous_commit = off` because log rows tolerate the loss window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;

use super::columns::LogColumns;
use crate::models::{LogEntry, LoggingError};

pub(in crate::repository) async fn insert_log_batch(
    pool: &PgPool,
    entries: &[LogEntry],
) -> Result<(), LoggingError> {
    let mut tx = pool.begin().await?;
    sqlx::query!("SET LOCAL synchronous_commit = off")
        .execute(&mut *tx)
        .await?;

    insert_batch(&mut tx, entries).await?;
    tx.commit().await?;
    Ok(())
}

async fn insert_batch(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entries: &[LogEntry],
) -> Result<(), LoggingError> {
    let columns = LogColumns::gather(entries)?;
    // Why: sqlx infers `&[String]` for a text[] bind; the nullable
    // columns need their element type stated once, without a cast.
    let metadata: &[Option<String>] = &columns.metadata;
    let task_ids: &[Option<String>] = &columns.task_ids;
    let context_ids: &[Option<String>] = &columns.context_ids;
    let client_ids: &[Option<String>] = &columns.client_ids;
    let instance_ids: &[Option<String>] = &columns.instance_ids;
    sqlx::query!(
        r"
            INSERT INTO logs (id, timestamp, level, module, message, metadata, user_id, session_id, task_id, trace_id, context_id, client_id, instance_id)
            SELECT * FROM UNNEST($1::text[], $2::timestamptz[], $3::text[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[], $9::text[], $10::text[], $11::text[], $12::text[], $13::text[])
            ",
        &columns.ids,
        &columns.timestamps,
        &columns.levels,
        &columns.modules,
        &columns.messages,
        metadata as _,
        &columns.user_ids,
        &columns.session_ids,
        task_ids as _,
        &columns.trace_ids,
        context_ids as _,
        client_ids as _,
        instance_ids as _
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
