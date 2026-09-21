use systemprompt_agent::repository::AgentOwnerReassignment;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use systemprompt_traits::OwnerReassignment;

async fn seed_graph(pool: &sqlx::PgPool, owner: &UserId, suffix: &str) {
    let context = format!("context-{suffix}");
    let task = format!("task-{suffix}");
    sqlx::query("INSERT INTO user_contexts(context_id,user_id,name) VALUES($1,$2,'owner graph')")
        .bind(&context)
        .bind(owner.as_str())
        .execute(pool)
        .await
        .expect("context");
    sqlx::query(
        "INSERT INTO agent_tasks(task_id,context_id,status,user_id) \
         VALUES($1,$2,'TASK_STATE_COMPLETED',$3)",
    )
    .bind(&task)
    .bind(&context)
    .bind(owner.as_str())
    .execute(pool)
    .await
    .expect("task");
    sqlx::query(
        "INSERT INTO task_messages(task_id,message_id,role,context_id,user_id,sequence_number) \
         VALUES($1,$2,'user',$3,$4,0)",
    )
    .bind(&task)
    .bind(format!("message-{suffix}"))
    .bind(&context)
    .bind(owner.as_str())
    .execute(pool)
    .await
    .expect("message");
}

async fn counts(pool: &sqlx::PgPool, owner: &UserId) -> (i64, i64, i64) {
    let contexts =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM user_contexts WHERE user_id=$1")
            .bind(owner.as_str())
            .fetch_one(pool)
            .await
            .expect("contexts");
    let tasks = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM agent_tasks WHERE user_id=$1")
        .bind(owner.as_str())
        .fetch_one(pool)
        .await
        .expect("tasks");
    let messages =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM task_messages WHERE user_id=$1")
            .bind(owner.as_str())
            .fetch_one(pool)
            .await
            .expect("messages");
    (contexts, tasks, messages)
}

#[tokio::test]
async fn late_message_failure_rolls_back_agent_graph_transfer_then_retry_moves_every_row() {
    let database = DisposableDb::installed("agent_owner_reassignment")
        .await
        .expect("isolated database");
    let db = database.pool().await.expect("database pool");
    let raw = db.write_pool_arc().expect("write pool");
    let source = UserId::new(format!("agent-source-{}", uuid::Uuid::new_v4()));
    let target = UserId::new(format!("agent-target-{}", uuid::Uuid::new_v4()));
    seed_user_row(&db, &source, &format!("{source}@agent-owner.invalid"))
        .await
        .expect("source");
    seed_user_row(&db, &target, &format!("{target}@agent-owner.invalid"))
        .await
        .expect("target");
    seed_graph(raw.as_ref(), &source, "source").await;
    seed_graph(raw.as_ref(), &target, "target").await;
    let reassignment = AgentOwnerReassignment::new(&db).expect("agent owner reassignment");

    sqlx::query(
        "CREATE FUNCTION reject_agent_message_reassignment() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF OLD.user_id <> NEW.user_id THEN RAISE EXCEPTION 'fixture message rejection'; \
         END IF; RETURN NEW; END $$",
    )
    .execute(raw.as_ref()).await.expect("failure function");
    sqlx::query(
        "CREATE TRIGGER reject_agent_message_reassignment BEFORE UPDATE OF user_id ON task_messages \
         FOR EACH ROW EXECUTE FUNCTION reject_agent_message_reassignment()",
    )
    .execute(raw.as_ref()).await.expect("failure trigger");

    let error = reassignment
        .reassign_owner(&source, &target)
        .await
        .expect_err("last graph update must roll back contexts and tasks");
    assert!(error.to_string().contains("fixture message rejection"));
    assert_eq!(counts(raw.as_ref(), &source).await, (1, 1, 1));
    assert_eq!(counts(raw.as_ref(), &target).await, (1, 1, 1));

    sqlx::query("DROP TRIGGER reject_agent_message_reassignment ON task_messages")
        .execute(raw.as_ref())
        .await
        .expect("remove trigger");
    sqlx::query("DROP FUNCTION reject_agent_message_reassignment()")
        .execute(raw.as_ref())
        .await
        .expect("remove function");
    let moved = reassignment
        .reassign_owner(&source, &target)
        .await
        .expect("retry owner reassignment");
    assert_eq!(reassignment.domain(), "agent");
    assert_eq!(
        moved.tables,
        vec![
            ("user_contexts", 1),
            ("agent_tasks", 1),
            ("task_messages", 1)
        ]
    );
    assert_eq!(moved.total(), 3);
    assert_eq!(counts(raw.as_ref(), &source).await, (0, 0, 0));
    assert_eq!(counts(raw.as_ref(), &target).await, (2, 2, 2));

    drop(reassignment);
    drop(raw);
    db.write_pool_arc().expect("write pool").close().await;
    drop(db);
    database.drop_now().await;
}
