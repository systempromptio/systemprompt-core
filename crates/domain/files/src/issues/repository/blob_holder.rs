//! The issue attachment table's registration with blob collection and user
//! purge: an attached blob is kept while a report holds it, and a deleted
//! user's reports leave with them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::future::BoxFuture;
use sqlx::PgConnection;
use systemprompt_extension::user_purge_tables;

fn held_among<'c>(
    conn: &'c mut PgConnection,
    hashes: &'c [String],
) -> BoxFuture<'c, Result<Vec<String>, sqlx::Error>> {
    Box::pin(async move {
        sqlx::query_scalar!(
            r#"SELECT DISTINCT content_sha256 AS "content_sha256!"
               FROM issue_report_attachments
               WHERE content_sha256 = ANY($1::text[])"#,
            hashes,
        )
        .fetch_all(conn)
        .await
    })
}

crate::register_blob_holder!("issue_report_attachments", held_among);

user_purge_tables!(
    "systemprompt-files",
    [("issue_reports", "reporter_user_id")]
);
