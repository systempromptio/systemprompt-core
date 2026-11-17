//! The issue attachment table's blob holder and user-purge row: an attached
//! blob is kept while a report holds it, and a deleted user's reports leave
//! with them. The files member contributes both.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::future::BoxFuture;
use sqlx::PgConnection;
use systemprompt_extension::UserPurgeTable;

use crate::documents::repository::BlobHolder;

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

pub const ISSUE_ATTACHMENTS_HOLDER: BlobHolder = BlobHolder {
    name: "issue_report_attachments",
    held_among,
};

pub(crate) const ISSUE_USER_PURGES: &[UserPurgeTable] = &[UserPurgeTable {
    table: "issue_reports",
    column: "reporter_user_id",
    predicate: None,
}];
