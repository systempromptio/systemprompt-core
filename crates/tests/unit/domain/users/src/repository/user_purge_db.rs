//! DB-backed tests for the user purge: the deleted user's rows leave with
//! them, content they shared with another user stays, and the deletion facts
//! this transaction's capture triggers write are delivered, not purged.

use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_users::{UserRepository, UserService};
use uuid::Uuid;

struct Ctx {
    fixture: crate::privacy_fixture::PrivacyFixture,
    service: UserService,
    raw: Arc<sqlx::PgPool>,
}

async fn setup_or_skip() -> Option<Ctx> {
    let fixture = crate::privacy_fixture::PrivacyFixture::new().await?;
    let raw = fixture.pool.pool_arc().expect("raw pool");
    let service = UserService::new(Arc::new(
        UserRepository::new(&fixture.pool).expect("user repository"),
    ));
    Some(Ctx {
        fixture,
        service,
        raw,
    })
}

async fn create_user(ctx: &Ctx, prefix: &str) -> UserId {
    let tag = Uuid::new_v4().simple().to_string();
    ctx.service
        .create(
            &format!("{prefix}-{tag}"),
            &format!("{prefix}-{tag}@purge.invalid"),
            Some("Purge Person"),
            None,
        )
        .await
        .expect("create user")
        .id
}

async fn seed_payload(ctx: &Ctx, sha256: &str) {
    sqlx::query(
        "INSERT INTO artifact_payloads (sha256, byte_len, body, ref_count) VALUES ($1, 2, '{}'::jsonb, 1)",
    )
    .bind(sha256)
    .execute(ctx.raw.as_ref())
    .await
    .expect("seed payload");
}

async fn seed_artifact(ctx: &Ctx, user: &UserId, sha256: &str) {
    let exec = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, input, user_id) \
         VALUES ($1, 'Read', 'purge-tests', NOW(), '{}', $2)",
    )
    .bind(&exec)
    .bind(user.as_str())
    .execute(ctx.raw.as_ref())
    .await
    .expect("seed execution");
    sqlx::query(
        "INSERT INTO mcp_artifacts (artifact_id, mcp_execution_id, user_id, server_name, artifact_type, data, payload_sha256) \
         VALUES ($1, $2, $3, 'purge-tests', 'tool_result', '{}'::jsonb, $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&exec)
    .bind(user.as_str())
    .bind(sha256)
    .execute(ctx.raw.as_ref())
    .await
    .expect("seed artifact");
}

async fn payload_exists(ctx: &Ctx, sha256: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM artifact_payloads WHERE sha256 = $1)",
    )
    .bind(sha256)
    .fetch_one(ctx.raw.as_ref())
    .await
    .expect("payload exists")
}

fn digest(tag: &str) -> String {
    format!("{:0>64}", Uuid::new_v4().simple().to_string() + tag)
}

#[tokio::test]
async fn purge_removes_the_users_own_payload_and_keeps_one_shared_with_another_user() {
    let Some(ctx) = setup_or_skip().await else {
        return;
    };
    let victim = create_user(&ctx, "purge-own").await;
    let other = create_user(&ctx, "purge-other").await;
    let own = digest("a");
    let shared = digest("b");
    seed_payload(&ctx, &own).await;
    seed_payload(&ctx, &shared).await;
    seed_artifact(&ctx, &victim, &own).await;
    seed_artifact(&ctx, &victim, &shared).await;
    seed_artifact(&ctx, &other, &shared).await;

    let preview = ctx.service.purge_preview(&victim).await.expect("preview");
    let previewed = preview
        .iter()
        .find(|c| c.table == "artifact_payloads")
        .expect("preview reports the payload sweep");
    assert_eq!(previewed.owner, "systemprompt-core");
    assert_eq!(
        previewed.rows, 1,
        "only the payload nothing but this user's artifacts reference is previewed"
    );

    ctx.fixture.drain().await;
    let removed = ctx.service.delete(&victim).await.expect("delete");
    let swept = removed
        .iter()
        .find(|c| c.table == "artifact_payloads")
        .expect("delete reports the payload sweep");
    assert_eq!(swept.rows, 1);
    assert!(
        !payload_exists(&ctx, &own).await,
        "the user's own body is gone"
    );
    assert!(
        payload_exists(&ctx, &shared).await,
        "a body another user's artifact still references stays"
    );

    ctx.fixture.drain().await;
    ctx.service.delete(&other).await.expect("cleanup other");
    assert!(
        !payload_exists(&ctx, &shared).await,
        "the last reference leaving takes the body with it"
    );
    ctx.fixture.finish().await;
}

#[tokio::test]
async fn purge_counts_only_the_users_non_reporting_outbox_rows() {
    let Some(ctx) = setup_or_skip().await else {
        return;
    };
    let victim = create_user(&ctx, "purge-outbox").await;
    let sha256 = digest("c");
    seed_payload(&ctx, &sha256).await;
    seed_artifact(&ctx, &victim, &sha256).await;
    sqlx::query(
        "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id) \
         VALUES ($1, 'tasks', $2, '{}'::jsonb, 'user', $2, 'purge-tests')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(victim.as_str())
    .execute(ctx.raw.as_ref())
    .await
    .expect("seed outbox event");

    ctx.fixture.drain().await;
    let removed = ctx.service.delete(&victim).await.expect("delete");
    let outbox = removed
        .iter()
        .find(|c| c.table == "event_outbox")
        .expect("delete reports the outbox purge");
    assert_eq!(
        outbox.rows, 1,
        "the deletion facts the capture triggers wrote in this transaction are not counted as purged"
    );
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM event_outbox WHERE user_id = $1 AND channel <> 'reporting'",
    )
    .bind(victim.as_str())
    .fetch_one(ctx.raw.as_ref())
    .await
    .expect("count");
    assert_eq!(remaining, 0);
    ctx.fixture.finish().await;
}
