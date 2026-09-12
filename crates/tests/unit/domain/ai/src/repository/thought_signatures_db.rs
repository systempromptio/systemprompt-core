// DB-backed tests for AiThoughtSignatureRepository.

use std::time::Duration;

use systemprompt_ai::repository::AiThoughtSignatureRepository;
use systemprompt_ai::repository::thought_signatures::ThoughtSignatureWrite;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{GatewayConversationId, UserId};

use super::pool_or_skip;

const TTL: Duration = Duration::from_secs(3600);

fn conversation() -> GatewayConversationId {
    GatewayConversationId::new_unchecked(&format!(
        "ctx_{:016x}",
        u64::from(uuid::Uuid::new_v4().as_u128() as u32)
    ))
}

async fn expire(pool: &DbPool, conversation: &GatewayConversationId, tool_use_id: &str) {
    let write = pool.write_pool_arc().unwrap();
    sqlx::query(
        "UPDATE ai_gateway_thought_signatures SET expires_at = NOW() - INTERVAL '1 hour' \
         WHERE conversation_id = $1 AND tool_use_id = $2",
    )
    .bind(conversation.as_str())
    .bind(tool_use_id)
    .execute(write.as_ref())
    .await
    .unwrap();
}

#[tokio::test]
async fn upsert_then_find_returns_the_signature() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let user_id = UserId::generate();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user_id.as_str())
        .bind(format!("{}@signature.test", user_id.as_str()))
        .execute(pool.write_pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    let repo = AiThoughtSignatureRepository::new(&pool).unwrap();
    let conv = conversation();

    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "call_1",
        signature: "sig-a",
        ttl: TTL,
    })
    .await
    .unwrap();

    assert_eq!(
        repo.find(&user_id, &conv, "call_1", TTL)
            .await
            .unwrap()
            .as_deref(),
        Some("sig-a")
    );
}

#[tokio::test]
async fn upsert_overwrites_an_existing_signature() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let user_id = UserId::generate();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user_id.as_str())
        .bind(format!("{}@signature.test", user_id.as_str()))
        .execute(pool.write_pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    let repo = AiThoughtSignatureRepository::new(&pool).unwrap();
    let conv = conversation();

    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "call_1",
        signature: "sig-a",
        ttl: TTL,
    })
    .await
    .unwrap();
    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "call_1",
        signature: "sig-b",
        ttl: TTL,
    })
    .await
    .unwrap();

    assert_eq!(
        repo.find(&user_id, &conv, "call_1", TTL)
            .await
            .unwrap()
            .as_deref(),
        Some("sig-b")
    );
}

#[tokio::test]
async fn find_is_scoped_to_the_conversation() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let user_id = UserId::generate();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user_id.as_str())
        .bind(format!("{}@signature.test", user_id.as_str()))
        .execute(pool.write_pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    let repo = AiThoughtSignatureRepository::new(&pool).unwrap();
    let conv = conversation();
    let other = conversation();

    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "call_1",
        signature: "sig-a",
        ttl: TTL,
    })
    .await
    .unwrap();

    assert!(
        repo.find(&user_id, &other, "call_1", TTL)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn expired_signature_is_not_found() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let user_id = UserId::generate();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user_id.as_str())
        .bind(format!("{}@signature.test", user_id.as_str()))
        .execute(pool.write_pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    let repo = AiThoughtSignatureRepository::new(&pool).unwrap();
    let conv = conversation();

    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "call_1",
        signature: "sig-a",
        ttl: TTL,
    })
    .await
    .unwrap();
    expire(&pool, &conv, "call_1").await;

    assert!(
        repo.find(&user_id, &conv, "call_1", TTL)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn find_extends_the_expiry() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let user_id = UserId::generate();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user_id.as_str())
        .bind(format!("{}@signature.test", user_id.as_str()))
        .execute(pool.write_pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    let repo = AiThoughtSignatureRepository::new(&pool).unwrap();
    let conv = conversation();

    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "call_1",
        signature: "sig-a",
        ttl: Duration::from_secs(1),
    })
    .await
    .unwrap();
    assert!(
        repo.find(&user_id, &conv, "call_1", TTL)
            .await
            .unwrap()
            .is_some()
    );

    let write = pool.write_pool_arc().unwrap();
    let remaining: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM (expires_at - NOW()))::FLOAT8 \
         FROM ai_gateway_thought_signatures WHERE conversation_id = $1 AND tool_use_id = $2",
    )
    .bind(conv.as_str())
    .bind("call_1")
    .fetch_one(write.as_ref())
    .await
    .unwrap();
    assert!(
        remaining > 3000.0,
        "expiry was not extended: {remaining}s left"
    );
}

#[tokio::test]
async fn cleanup_expired_removes_only_expired_rows() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let user_id = UserId::generate();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user_id.as_str())
        .bind(format!("{}@signature.test", user_id.as_str()))
        .execute(pool.write_pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    let repo = AiThoughtSignatureRepository::new(&pool).unwrap();
    let conv = conversation();

    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "live",
        signature: "sig-a",
        ttl: TTL,
    })
    .await
    .unwrap();
    repo.upsert(&ThoughtSignatureWrite {
        user_id: &user_id,
        conversation: &conv,
        tool_use_id: "stale",
        signature: "sig-b",
        ttl: TTL,
    })
    .await
    .unwrap();
    expire(&pool, &conv, "stale").await;

    let removed = repo.cleanup_expired().await.unwrap();
    assert!(
        removed >= 1,
        "expected at least the stale row, got {removed}"
    );
    assert!(
        repo.find(&user_id, &conv, "live", TTL)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        repo.find(&user_id, &conv, "stale", TTL)
            .await
            .unwrap()
            .is_none()
    );
}
