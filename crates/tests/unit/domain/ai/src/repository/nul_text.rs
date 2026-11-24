// Text and JSON carrying a NUL is stored with U+FFFD in its place: PostgreSQL
// refuses a NUL in text and jsonb, which failed the whole write.

use serde_json::{Value, json};
use systemprompt_ai::models::{ArgsCheck, ArgsValidity, RequestStatus};
use systemprompt_ai::repository::ai_requests::{
    SettleCompletion, SettledFailure, SettledToolCall, SettlementOutcome, SettlementUsage,
    UsageSource,
};
use systemprompt_ai::repository::{
    AiRequestPayloadRepository, AiRequestRepository, AiSafetyFindingRepository,
    InsertSafetyFinding, InsertToolCallParams, RequestMessageRow, UpsertPayloadParams,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, AiToolCallId, McpToolName};

use super::{bootstrapped_pool, seed_request, user};

const RAW: &str = "a\0b";
const STORED: &str = "a\u{FFFD}b";

async fn text(pool: &DbPool, sql: &'static str, id: &AiRequestId) -> Vec<Option<String>> {
    sqlx::query_scalar(sql)
        .bind(id.as_str())
        .fetch_all(pool.pool().as_ref())
        .await
        .expect("stored text")
}

async fn jsonb(pool: &DbPool, sql: &'static str, id: &AiRequestId) -> Value {
    sqlx::query_scalar(sql)
        .bind(id.as_str())
        .fetch_one(pool.pool().as_ref())
        .await
        .expect("stored json")
}

fn stored(n: usize) -> Vec<Option<String>> {
    vec![Some(STORED.to_owned()); n]
}

fn bad_args() -> ArgsCheck {
    ArgsCheck {
        validity: ArgsValidity::Invalid,
        error_path: Some(RAW.to_owned()),
    }
}

#[tokio::test]
async fn a_completion_carrying_nul_settles() {
    let pool = bootstrapped_pool().await;
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let body = json!({"content": RAW, RAW: 1});
    let tools = [SettledToolCall {
        id: AiToolCallId::new("toolu_nul"),
        name: RAW.to_owned(),
        input: RAW.to_owned(),
        args: Some(bad_args()),
    }];
    AiRequestRepository::new(&pool)
        .settle(
            &id,
            &uid,
            SettlementOutcome::Completed(SettleCompletion {
                usage: SettlementUsage::default(),
                cost_microdollars: 0,
                latency_ms: 1,
                upstream_latency_ms: None,
                finish_reason: Some(RAW),
                payload: UpsertPayloadParams {
                    body: Some(&body),
                    excerpt: Some(RAW),
                    truncated: false,
                    bytes: Some(1),
                    sha256: Some("sha-nul"),
                },
                assistant_text: Some(RAW),
                tool_calls: &tools,
                next_message_sequence: Some(0),
                usage_source: UsageSource::Reported,
            }),
        )
        .await
        .expect("settle");

    for sql in [
        "SELECT finish_reason FROM ai_requests WHERE id = $1",
        "SELECT response_excerpt FROM ai_request_payloads WHERE ai_request_id = $1",
        "SELECT content FROM ai_request_messages WHERE request_id = $1",
        "SELECT tool_name FROM ai_request_tool_calls WHERE request_id = $1",
        "SELECT tool_input FROM ai_request_tool_calls WHERE request_id = $1",
        "SELECT args_error_path FROM ai_request_tool_calls WHERE request_id = $1",
    ] {
        assert_eq!(text(&pool, sql, &id).await, stored(1), "{sql}");
    }
    assert_eq!(
        jsonb(
            &pool,
            "SELECT response_body FROM ai_request_payloads WHERE ai_request_id = $1",
            &id
        )
        .await,
        json!({"content": STORED, STORED: 1})
    );
}

#[tokio::test]
async fn request_payloads_and_tool_catalogs_carrying_nul_are_stored() {
    let pool = bootstrapped_pool().await;
    let id = seed_request(&pool, &user()).await;
    let repo = AiRequestPayloadRepository::new(&pool);
    let body = json!({"prompt": RAW});
    let tools = json!([{"name": RAW, "probe": id.as_str()}]);
    let clean = json!([{"name": STORED, "probe": id.as_str()}]);
    repo.upsert_request(
        &id,
        UpsertPayloadParams {
            body: Some(&body),
            excerpt: Some(RAW),
            truncated: false,
            bytes: Some(1),
            sha256: Some("sha-nul"),
        },
    )
    .await
    .expect("request payload");
    repo.upsert_offered_tools(&id, &tools)
        .await
        .expect("offered tools");
    repo.upsert_prepared(&id, "prepared", Some(&tools))
        .await
        .expect("prepared");

    assert_eq!(
        text(
            &pool,
            "SELECT request_excerpt FROM ai_request_payloads WHERE ai_request_id = $1",
            &id
        )
        .await,
        stored(1)
    );
    for (sql, expected) in [
        (
            "SELECT request_body FROM ai_request_payloads WHERE ai_request_id = $1",
            json!({"prompt": STORED}),
        ),
        (
            "SELECT c.tools FROM ai_request_payloads p JOIN ai_tool_catalogs c \
             ON c.sha256 = p.offered_tools_sha256 WHERE p.ai_request_id = $1",
            clean.clone(),
        ),
        (
            "SELECT c.tools FROM ai_request_payloads p JOIN ai_tool_catalogs c \
             ON c.sha256 = p.prepared_tools_sha256 WHERE p.ai_request_id = $1",
            clean,
        ),
    ] {
        assert_eq!(jsonb(&pool, sql, &id).await, expected, "{sql}");
    }
}

#[tokio::test]
async fn messages_tool_calls_and_findings_carrying_nul_are_stored() {
    let pool = bootstrapped_pool().await;
    let id = seed_request(&pool, &user()).await;
    let repo = AiRequestRepository::new(&pool);
    repo.insert_message(&id, "user", RAW, 0)
        .await
        .expect("message");
    repo.insert_messages(
        &id,
        &[RequestMessageRow {
            role: "user",
            content: RAW,
            sequence_number: 1,
        }],
    )
    .await
    .expect("messages");
    let args = bad_args();
    repo.insert_tool_call(InsertToolCallParams {
        request_id: &id,
        ai_tool_call_id: &AiToolCallId::new("toolu_nul"),
        tool_name: &McpToolName::new("lookup"),
        tool_input: RAW,
        sequence_number: 1,
        args: Some(&args),
    })
    .await
    .expect("tool call");
    let findings = AiSafetyFindingRepository::new(&pool);
    let finding = InsertSafetyFinding {
        ai_request_id: &id,
        phase: "input",
        severity: "low",
        category: "pii",
        scanner: "heuristic",
        excerpt: Some(RAW),
        blocked: false,
    };
    findings.insert(finding.clone()).await.expect("finding");
    findings.insert_many(&[finding]).await.expect("findings");

    for (sql, n) in [
        (
            "SELECT content FROM ai_request_messages WHERE request_id = $1",
            2,
        ),
        (
            "SELECT tool_input FROM ai_request_tool_calls WHERE request_id = $1",
            1,
        ),
        (
            "SELECT args_error_path FROM ai_request_tool_calls WHERE request_id = $1",
            1,
        ),
        (
            "SELECT excerpt FROM ai_safety_findings WHERE ai_request_id = $1",
            2,
        ),
    ] {
        assert_eq!(text(&pool, sql, &id).await, stored(n), "{sql}");
    }
}

#[tokio::test]
async fn errors_carrying_nul_are_recorded() {
    let pool = bootstrapped_pool().await;
    let repo = AiRequestRepository::new(&pool);
    let message = "SELECT error_message FROM ai_requests WHERE id = $1";

    let uid = user();
    let settled = seed_request(&pool, &uid).await;
    repo.settle(
        &settled,
        &uid,
        SettlementOutcome::Failed(SettledFailure {
            error: RAW,
            ..Default::default()
        }),
    )
    .await
    .expect("failed settlement");
    assert_eq!(text(&pool, message, &settled).await, stored(1));

    let uid = user();
    let unaccounted = seed_request(&pool, &uid).await;
    for _ in 0..2 {
        repo.mark_accounting_failed(&unaccounted, &uid, RAW)
            .await
            .expect("a repeated mark matches the stored diagnostic");
    }
    assert_eq!(text(&pool, message, &unaccounted).await, stored(1));

    let updated = seed_request(&pool, &user()).await;
    repo.update_error(&updated, RequestStatus::Failed, RAW)
        .await
        .expect("update error");
    assert_eq!(text(&pool, message, &updated).await, stored(1));
}
