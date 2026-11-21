//! An account merge renders every admitted owner reassignment from its
//! declaration and runs them all, with the users-owned half, in one
//! transaction: a fault in any declared table leaves every row and the
//! source user in place, and a rerun completes the merge.

use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{DisposableDb, merging_user_service, seed_user_row};
use systemprompt_users::UserError;

async fn exec(pool: &sqlx::PgPool, sql: &str, binds: &[&str]) {
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()));
    for bind in binds {
        query = query.bind(*bind);
    }
    query.execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn count(pool: &sqlx::PgPool, sql: &str, owner: &UserId) -> i64 {
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql.to_owned()))
        .bind(owner.as_str())
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn seed_owned_rows(pool: &sqlx::PgPool, owner: &UserId, suffix: &str) {
    let context = format!("context-{suffix}");
    let task = format!("task-{suffix}");
    exec(
        pool,
        "INSERT INTO user_contexts(context_id,user_id,name) VALUES($1,$2,'merge graph')",
        &[&context, owner.as_str()],
    )
    .await;
    exec(
        pool,
        "INSERT INTO agent_tasks(task_id,context_id,status,user_id) \
         VALUES($1,$2,'TASK_STATE_COMPLETED',$3)",
        &[&task, &context, owner.as_str()],
    )
    .await;
    exec(
        pool,
        "INSERT INTO task_messages(task_id,message_id,role,context_id,user_id,sequence_number) \
         VALUES($1,$2,'user',$3,$4,0)",
        &[&task, &format!("message-{suffix}"), &context, owner.as_str()],
    )
    .await;
    exec(
        pool,
        "INSERT INTO mcp_tool_executions \
         (mcp_execution_id,tool_name,server_name,started_at,input,status,user_id) \
         VALUES ($1,'read','fixture',NOW(),'{}','success',$2)",
        &[&format!("exec-{suffix}"), owner.as_str()],
    )
    .await;
    exec(
        pool,
        "INSERT INTO mcp_external_sessions (server_name,session_id,user_id,credential_hash) \
         VALUES ('fixture',$1,$2,convert_to($1,'UTF8'))",
        &[&format!("external-{suffix}"), owner.as_str()],
    )
    .await;
    exec(
        pool,
        "INSERT INTO ai_quota_buckets (id,subject_id,subject_kind,window_seconds,window_start) \
         VALUES ($1,$2,'user',60,NOW())",
        &[&format!("quota-{suffix}"), owner.as_str()],
    )
    .await;
    exec(
        pool,
        "INSERT INTO ai_gateway_thought_signatures \
         (user_id,conversation_id,tool_use_id,signature,expires_at) \
         VALUES ($1,'shared-conversation','shared-tool',$2,NOW() + INTERVAL '1 hour')",
        &[owner.as_str(), &format!("signature-{suffix}")],
    )
    .await;
}

const CONTEXTS: &str = "SELECT count(*) FROM user_contexts WHERE user_id=$1";
const TASKS: &str = "SELECT count(*) FROM agent_tasks WHERE user_id=$1";
const MESSAGES: &str = "SELECT count(*) FROM task_messages WHERE user_id=$1";
const EXECUTIONS: &str = "SELECT count(*) FROM mcp_tool_executions WHERE user_id=$1";
const EXTERNAL: &str = "SELECT count(*) FROM mcp_external_sessions WHERE user_id=$1";
const QUOTAS: &str =
    "SELECT count(*) FROM ai_quota_buckets WHERE subject_kind='user' AND subject_id=$1";
const USERS: &str = "SELECT count(*) FROM users WHERE id=$1";

async fn owned(pool: &sqlx::PgPool, owner: &UserId) -> [i64; 6] {
    [
        count(pool, CONTEXTS, owner).await,
        count(pool, TASKS, owner).await,
        count(pool, MESSAGES, owner).await,
        count(pool, EXECUTIONS, owner).await,
        count(pool, EXTERNAL, owner).await,
        count(pool, QUOTAS, owner).await,
    ]
}

#[tokio::test]
async fn a_fault_in_one_declared_table_rolls_back_the_whole_merge_and_a_rerun_completes_it() {
    let database = DisposableDb::with_schema("owner_reassignment_merge").await;
    let db = database.test_pool().await;
    let raw = db.write_pool();
    let source = UserId::new(format!("merge-source-{}", uuid::Uuid::new_v4()));
    let target = UserId::new(format!("merge-target-{}", uuid::Uuid::new_v4()));
    for owner in [&source, &target] {
        seed_user_row(&db, owner, &format!("{owner}@merge.invalid"))
            .await
            .expect("seed user");
    }
    seed_owned_rows(raw.as_ref(), &source, "source").await;
    seed_owned_rows(raw.as_ref(), &target, "target").await;
    let service = merging_user_service(&db).expect("merging user service");

    sqlx::raw_sql(sqlx::AssertSqlSafe(
        "CREATE FUNCTION reject_message_reassignment() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'fixture message rejection'; END $$; \
         CREATE TRIGGER reject_message_reassignment BEFORE UPDATE OF user_id ON task_messages \
         FOR EACH ROW EXECUTE FUNCTION reject_message_reassignment()",
    ))
    .execute(raw.as_ref())
    .await
    .expect("install fault");

    let error = service
        .merge_users(&source, &target)
        .await
        .expect_err("a fault in task_messages aborts the merge");
    assert!(
        matches!(
            &error,
            UserError::OwnerReassignment { owner, table, column, .. }
                if owner.as_str() == "agent" && table == "task_messages" && column == "user_id"
        ),
        "{error:?}"
    );
    assert_eq!(owned(raw.as_ref(), &source).await, [1; 6], "nothing moved");
    assert_eq!(owned(raw.as_ref(), &target).await, [1; 6], "nothing moved");
    assert_eq!(count(raw.as_ref(), USERS, &source).await, 1);

    sqlx::raw_sql(sqlx::AssertSqlSafe(
        "DROP TRIGGER reject_message_reassignment ON task_messages; \
         DROP FUNCTION reject_message_reassignment()",
    ))
    .execute(raw.as_ref())
    .await
    .expect("remove fault");

    let merged = service
        .merge_users(&source, &target)
        .await
        .expect("the rerun completes the merge");
    assert_eq!(merged.tasks, 1);
    assert_eq!(count(raw.as_ref(), USERS, &source).await, 0);
    assert_eq!(owned(raw.as_ref(), &target).await, [2, 2, 2, 2, 1, 1]);
    let kept: String = sqlx::query_scalar(
        "SELECT signature FROM ai_gateway_thought_signatures \
         WHERE user_id=$1 AND conversation_id='shared-conversation' AND tool_use_id='shared-tool'",
    )
    .bind(target.as_str())
    .fetch_one(raw.as_ref())
    .await
    .expect("target signature");
    assert_eq!(kept, "signature-target", "the target's own row is kept");

    drop(service);
    drop(raw);
    db.write_pool().close().await;
    drop(db);
    database.drop_now().await;
}
