use systemprompt_identifiers::UserId;
use systemprompt_mcp::repository::McpOwnerReassignment;
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use systemprompt_traits::OwnerReassignment;

async fn seed_owned_rows(pool: &sqlx::PgPool, owner: &UserId, suffix: &str) {
    let execution = format!("exec-{suffix}");
    let session = format!("session-{suffix}");
    sqlx::query(
        "INSERT INTO mcp_tool_executions \
         (mcp_execution_id,tool_name,server_name,started_at,input,status,user_id) \
         VALUES ($1,'read','fixture',NOW(),'{}','success',$2)",
    )
    .bind(&execution)
    .bind(owner.as_str())
    .execute(pool)
    .await
    .expect("tool execution");
    sqlx::query(
        "INSERT INTO mcp_artifacts \
         (artifact_id,mcp_execution_id,user_id,server_name,tool_name,artifact_type,data) \
         VALUES ($1,$2,$3,'fixture','read','tool_result','{}'::jsonb)",
    )
    .bind(format!("artifact-{suffix}"))
    .bind(&execution)
    .bind(owner.as_str())
    .execute(pool)
    .await
    .expect("artifact");
    sqlx::query(
        "INSERT INTO mcp_sessions (session_id,user_id,mcp_server_id,status) \
         VALUES ($1,$2,'fixture','active')",
    )
    .bind(&session)
    .bind(owner.as_str())
    .execute(pool)
    .await
    .expect("MCP session");
    sqlx::query(
        "INSERT INTO mcp_proxy_identities \
         (session_id,user_id,user_type,permissions,roles,auth_token) \
         VALUES ($1,$2,'user','[]'::jsonb,'[]'::jsonb,$3)",
    )
    .bind(format!("proxy-{suffix}"))
    .bind(owner.as_str())
    .bind(format!("token-{suffix}"))
    .execute(pool)
    .await
    .expect("proxy identity");
    sqlx::query(
        "INSERT INTO mcp_external_sessions \
         (server_name,session_id,user_id,credential_hash) VALUES ('fixture',$1,$2,$3)",
    )
    .bind(format!("external-{suffix}"))
    .bind(owner.as_str())
    .bind(format!("credential-{suffix}").into_bytes())
    .execute(pool)
    .await
    .expect("external session");
}

async fn owner_count(pool: &sqlx::PgPool, table: &str, owner: &UserId) -> i64 {
    let query = match table {
        "mcp_tool_executions" => "SELECT count(*) FROM mcp_tool_executions WHERE user_id=$1",
        "mcp_artifacts" => "SELECT count(*) FROM mcp_artifacts WHERE user_id=$1",
        "mcp_sessions" => "SELECT count(*) FROM mcp_sessions WHERE user_id=$1",
        "mcp_proxy_identities" => "SELECT count(*) FROM mcp_proxy_identities WHERE user_id=$1",
        "mcp_external_sessions" => "SELECT count(*) FROM mcp_external_sessions WHERE user_id=$1",
        _ => panic!("unknown fixture table"),
    };
    sqlx::query_scalar::<_, i64>(query)
        .bind(owner.as_str())
        .fetch_one(pool)
        .await
        .expect("owner row count")
}

#[tokio::test]
async fn late_session_failure_rolls_back_mcp_owner_transfer_then_retry_moves_and_revokes() {
    let database = DisposableDb::installed("mcp_owner_reassignment")
        .await
        .expect("isolated database");
    let db = database.pool().await.expect("database pool");
    let raw = db.write_pool_arc().expect("write pool");
    let source = UserId::new(format!("mcp-source-{}", uuid::Uuid::new_v4()));
    let target = UserId::new(format!("mcp-target-{}", uuid::Uuid::new_v4()));
    seed_user_row(&db, &source, &format!("{source}@mcp-owner.invalid"))
        .await
        .expect("source owner");
    seed_user_row(&db, &target, &format!("{target}@mcp-owner.invalid"))
        .await
        .expect("target owner");
    seed_owned_rows(raw.as_ref(), &source, "source").await;
    seed_owned_rows(raw.as_ref(), &target, "target").await;
    let reassignment = McpOwnerReassignment::new(&db).expect("MCP owner reassignment");

    sqlx::query(
        "CREATE FUNCTION reject_mcp_session_reassignment() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF OLD.user_id <> NEW.user_id THEN RAISE EXCEPTION 'fixture session rejection'; \
         END IF; RETURN NEW; END $$",
    )
    .execute(raw.as_ref())
    .await
    .expect("failure function");
    sqlx::query(
        "CREATE TRIGGER reject_mcp_session_reassignment BEFORE UPDATE OF user_id ON mcp_sessions \
         FOR EACH ROW EXECUTE FUNCTION reject_mcp_session_reassignment()",
    )
    .execute(raw.as_ref())
    .await
    .expect("failure trigger");

    let error = reassignment
        .reassign_owner(&source, &target)
        .await
        .expect_err("third transfer failure must roll back earlier table updates");
    assert!(error.to_string().contains("fixture session rejection"));
    for table in [
        "mcp_tool_executions",
        "mcp_artifacts",
        "mcp_sessions",
        "mcp_proxy_identities",
        "mcp_external_sessions",
    ] {
        assert_eq!(
            owner_count(raw.as_ref(), table, &source).await,
            1,
            "{table}"
        );
        assert_eq!(
            owner_count(raw.as_ref(), table, &target).await,
            1,
            "{table}"
        );
    }

    sqlx::query("DROP TRIGGER reject_mcp_session_reassignment ON mcp_sessions")
        .execute(raw.as_ref())
        .await
        .expect("remove trigger");
    sqlx::query("DROP FUNCTION reject_mcp_session_reassignment()")
        .execute(raw.as_ref())
        .await
        .expect("remove function");

    let moved = reassignment
        .reassign_owner(&source, &target)
        .await
        .expect("owner reassignment retry");
    assert_eq!(reassignment.domain(), "mcp");
    assert_eq!(
        moved.tables,
        vec![
            ("mcp_tool_executions", 1),
            ("mcp_artifacts", 1),
            ("mcp_sessions", 1),
            ("mcp_proxy_identities", 1),
            ("mcp_external_sessions", 1),
        ]
    );
    assert_eq!(moved.total(), 5);
    for table in ["mcp_tool_executions", "mcp_artifacts", "mcp_sessions"] {
        assert_eq!(
            owner_count(raw.as_ref(), table, &source).await,
            0,
            "{table}"
        );
        assert_eq!(
            owner_count(raw.as_ref(), table, &target).await,
            2,
            "{table}"
        );
    }
    for table in ["mcp_proxy_identities", "mcp_external_sessions"] {
        assert_eq!(
            owner_count(raw.as_ref(), table, &source).await,
            0,
            "{table}"
        );
        assert_eq!(
            owner_count(raw.as_ref(), table, &target).await,
            1,
            "target credential state must remain while source credentials are revoked: {table}"
        );
    }

    drop(reassignment);
    drop(raw);
    db.write_pool_arc().expect("write pool").close().await;
    drop(db);
    database.drop_now().await;
}
