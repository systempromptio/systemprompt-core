//! An edit that Postgres aborts as the victim of a deadlock with a schema
//! install lands on its retry instead of failing the tool call.
//!
//! The install side is played by one connection taking the locks a full
//! install's `CREATE INDEX IF NOT EXISTS` statements take, in the install's
//! table order: `SHARE` on `documents`, then `SHARE` on
//! `document_revisions`. The edit is started only once that first lock is
//! held, so it takes `ROW EXCLUSIVE` on `document_revisions` and then waits
//! on `documents`; the install's second request closes the cycle. The
//! database's deadlock counter shows the cycle formed and was broken.

use std::time::Duration;

use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::EditOp;
use systemprompt_runtime::documents::mcp::{EditHandler, EditInput};
use systemprompt_test_fixtures::DisposableDb;

use crate::documents_mcp_support::{create_text, ctx_for, exec, harness_on, registered_on};

async fn deadlocks(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT deadlocks FROM pg_stat_database WHERE datname = current_database()")
        .fetch_one(pool)
        .await
        .expect("read the deadlock counter")
}

async fn waits_on_documents(pool: &sqlx::PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks l JOIN pg_class c ON c.oid = l.relation \
         WHERE c.relname = 'documents' AND NOT l.granted AND l.mode = 'RowExclusiveLock')",
    )
    .fetch_one(pool)
    .await
    .expect("read pg_locks")
}

#[tokio::test]
async fn an_edit_aborted_by_a_deadlock_with_a_schema_install_lands_on_its_retry() {
    let database = DisposableDb::with_schema("documents_deadlock_retry").await;
    let db = database.test_pool().await;
    let raw = db.write_pool().as_ref().clone();
    let h = harness_on(&db);
    let owner = registered_on(&db, "mcp-deadlock").await;
    let ctx = ctx_for(Some(&owner));
    let (document, head) = create_text(&h.tools, &ctx, "before\n").await;

    let mut install = raw.begin().await.expect("install transaction");
    sqlx::query("LOCK TABLE documents IN SHARE MODE")
        .execute(&mut *install)
        .await
        .expect("the install's first index lock");
    let before = deadlocks(&raw).await;

    let edit = {
        let tools = h.tools.clone();
        let ctx = ctx.clone();
        let document = document.clone();
        tokio::spawn(async move {
            EditHandler { tools: &tools }
                .handle(
                    EditInput {
                        document_id: document,
                        base_revision: Some(head),
                        mode: None,
                        op_id: None,
                        answering: None,
                        allow_detach: false,
                        ops: vec![EditOp::SetText {
                            text: "after\n".to_owned(),
                        }],
                        message: Some("racing the install".to_owned()),
                        verbose: false,
                        content_blob_ref: None,
                        context_id: None,
                    },
                    &ctx,
                    &exec(),
                )
                .await
                .map(|(out, _)| out.revision_id)
        })
    };
    for _ in 0..200 {
        if waits_on_documents(&raw).await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(
        waits_on_documents(&raw).await,
        "the edit holds its revision lock and waits on the install's documents lock"
    );
    sqlx::query("LOCK TABLE document_revisions IN SHARE MODE")
        .execute(&mut *install)
        .await
        .expect("the install's second index lock, granted once the edit is aborted");
    tokio::time::sleep(Duration::from_millis(100)).await;
    install.commit().await.expect("the install commits");

    let landed = edit
        .await
        .expect("the edit task")
        .expect("the aborted edit is run again and lands");
    assert_eq!(
        deadlocks(&raw).await - before,
        1,
        "exactly one deadlock formed, and the edit was its victim"
    );
    let head_now: String =
        sqlx::query_scalar("SELECT head_revision_id::text FROM documents WHERE id = $1::uuid")
            .bind(document.as_str())
            .fetch_one(&raw)
            .await
            .expect("the document head");
    assert_eq!(head_now, landed.as_str());
    database.drop_now().await;
}
