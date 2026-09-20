use chrono::Utc;
use std::time::Duration;
use systemprompt_ai::repository::thought_signatures::ThoughtSignatureWrite;
use systemprompt_ai::repository::{
    AiOwnerReassignment, AiQuotaBucketRepository, AiRequestRepository,
    AiThoughtSignatureRepository, IncrementParams, QuotaBucketDelta,
};
use systemprompt_identifiers::{GatewayConversationId, UserId};
use systemprompt_traits::OwnerReassignment;
use uuid::Uuid;

use super::{completed_record, pool_or_skip, user};

#[tokio::test]
async fn reassigning_an_owner_moves_requests_and_signatures_but_discards_source_quota() {
    let pool = pool_or_skip().await.expect("AI ownership fixture database");
    let source = user();
    let target = user();
    for owner in [&source, &target] {
        systemprompt_test_fixtures::seed_user_row(&pool, owner, &format!("{owner}@ai.invalid"))
            .await
            .unwrap();
    }
    let requests = AiRequestRepository::new(&pool).unwrap();
    requests.insert(&completed_record(&source)).await.unwrap();
    requests.insert(&completed_record(&source)).await.unwrap();
    let quotas = AiQuotaBucketRepository::new(&pool).unwrap();
    quotas
        .increment(IncrementParams {
            subject_kind: "user",
            subject_id: source.as_str(),
            window_seconds: 60,
            window_start: Utc::now(),
            delta: QuotaBucketDelta {
                requests: 2,
                input_tokens: 20,
                output_tokens: 8,
                cost_microdollars: 40,
            },
        })
        .await
        .unwrap();
    let signatures = AiThoughtSignatureRepository::new(&pool).unwrap();
    let conversation = GatewayConversationId::from_prefix_hash(Uuid::new_v4().as_u128() as u64);
    signatures
        .upsert(&ThoughtSignatureWrite {
            user_id: &source,
            conversation: &conversation,
            tool_use_id: "tool-1",
            signature: "signed",
            ttl: Duration::from_secs(60),
        })
        .await
        .unwrap();

    let moved = AiOwnerReassignment::new(&pool)
        .unwrap()
        .reassign_owner(&source, &target)
        .await
        .unwrap();
    let read = pool.pool_arc().unwrap();
    let request_count = sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM ai_requests WHERE user_id=$1",
        target.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .unwrap();
    let source_quota = sqlx::query_scalar!("SELECT count(*) AS \"n!\" FROM ai_quota_buckets WHERE subject_kind='user' AND subject_id=$1", source.as_str()).fetch_one(read.as_ref()).await.unwrap();

    assert_eq!(
        moved.tables,
        vec![
            ("ai_requests", 2),
            ("ai_quota_buckets", 1),
            ("ai_gateway_thought_signatures", 1)
        ]
    );
    assert_eq!(request_count, 2);
    assert_eq!(source_quota, 0);
    assert_eq!(
        signatures
            .find(&target, &conversation, "tool-1", Duration::from_secs(60))
            .await
            .unwrap()
            .as_deref(),
        Some("signed")
    );
    assert!(
        signatures
            .find(&source, &conversation, "tool-1", Duration::from_secs(60))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn signature_update_fault_rolls_back_owner_reassignment_across_all_ai_tables() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let database = systemprompt_test_fixtures::DisposableDb::installed("ai_owner_signature_fault")
        .await
        .expect("private AI database");
    let pool = database.pool().await.expect("private AI pool");
    let source = user();
    let target = user();
    for owner in [&source, &target] {
        systemprompt_test_fixtures::seed_user_row(&pool, owner, &format!("{owner}@ai.invalid"))
            .await
            .expect("seed user");
    }
    AiRequestRepository::new(&pool)
        .expect("requests repository")
        .insert(&completed_record(&source))
        .await
        .expect("source request");
    AiQuotaBucketRepository::new(&pool)
        .expect("quota repository")
        .increment(IncrementParams {
            subject_kind: "user",
            subject_id: source.as_str(),
            window_seconds: 60,
            window_start: Utc::now(),
            delta: QuotaBucketDelta {
                requests: 1,
                input_tokens: 1,
                output_tokens: 1,
                cost_microdollars: 1,
            },
        })
        .await
        .expect("source quota");
    let signatures = AiThoughtSignatureRepository::new(&pool).expect("signature repository");
    let conversation = GatewayConversationId::from_prefix_hash(Uuid::new_v4().as_u128() as u64);
    signatures
        .upsert(&ThoughtSignatureWrite {
            user_id: &source,
            conversation: &conversation,
            tool_use_id: "rollback-tool",
            signature: "source signature",
            ttl: Duration::from_secs(60),
        })
        .await
        .expect("source signature");
    let writer = pool.pool_arc().expect("private SQL pool");
    sqlx::raw_sql(sqlx::AssertSqlSafe("CREATE FUNCTION reject_owner_signature_reassignment() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture signature owner update rejection'; END $$; CREATE TRIGGER reject_owner_signature_reassignment BEFORE UPDATE OF user_id ON ai_gateway_thought_signatures FOR EACH ROW EXECUTE FUNCTION reject_owner_signature_reassignment()"))
        .execute(writer.as_ref()).await.expect("install private signature fault");

    let error = AiOwnerReassignment::new(&pool)
        .expect("owner reassignment")
        .reassign_owner(&source, &target)
        .await
        .expect_err("signature fault must abort reassignment");
    assert!(
        error
            .to_string()
            .contains("fixture signature owner update rejection"),
        "{error}"
    );
    let count = |owner: &UserId| {
        let owner = owner.as_str().to_owned();
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ai_requests WHERE user_id = $1")
            .bind(owner)
            .fetch_one(writer.as_ref())
    };
    assert_eq!(
        (
            count(&source).await.expect("source request count"),
            count(&target).await.expect("target request count")
        ),
        (1, 0)
    );
    let source_quota: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_quota_buckets WHERE subject_kind = 'user' AND subject_id = $1",
    )
    .bind(source.as_str())
    .fetch_one(writer.as_ref())
    .await
    .expect("source quota count");
    assert_eq!(source_quota, 1);
    assert_eq!(
        signatures
            .find(
                &source,
                &conversation,
                "rollback-tool",
                Duration::from_secs(60)
            )
            .await
            .expect("source signature lookup")
            .as_deref(),
        Some("source signature")
    );
    assert!(
        signatures
            .find(
                &target,
                &conversation,
                "rollback-tool",
                Duration::from_secs(60)
            )
            .await
            .expect("target signature lookup")
            .is_none()
    );

    sqlx::raw_sql(sqlx::AssertSqlSafe("DROP TRIGGER reject_owner_signature_reassignment ON ai_gateway_thought_signatures; DROP FUNCTION reject_owner_signature_reassignment()"))
        .execute(writer.as_ref()).await.expect("remove private signature fault");
    let moved = AiOwnerReassignment::new(&pool)
        .expect("owner reassignment")
        .reassign_owner(&source, &target)
        .await
        .expect("the same owners recover after the signature store is repaired");
    assert_eq!(
        moved.tables,
        vec![
            ("ai_requests", 1),
            ("ai_quota_buckets", 1),
            ("ai_gateway_thought_signatures", 1)
        ]
    );
    assert_eq!(
        (
            count(&source)
                .await
                .expect("recovered source request count"),
            count(&target)
                .await
                .expect("recovered target request count")
        ),
        (0, 1)
    );
    let source_quota: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ai_quota_buckets WHERE subject_kind = 'user' AND subject_id = $1",
    )
    .bind(source.as_str())
    .fetch_one(writer.as_ref())
    .await
    .expect("recovered source quota count");
    assert_eq!(source_quota, 0);
    assert_eq!(
        signatures
            .find(
                &target,
                &conversation,
                "rollback-tool",
                Duration::from_secs(60)
            )
            .await
            .expect("recovered target signature lookup")
            .as_deref(),
        Some("source signature")
    );

    drop(writer);
    drop(signatures);
    pool.pool_arc().expect("private SQL pool").close().await;
    drop(pool);
    database.drop_now().await;
}
