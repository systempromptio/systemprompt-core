//! Persists rejected gateway requests for audit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use bytes::Bytes;
use systemprompt_ai::models::RequestStatus;
use systemprompt_ai::models::ai_request_record::AiRequestRecord;
use systemprompt_ai::repository::{
    AiRequestPayloadRepository, AiRequestRepository, UpsertPayloadParams,
};
use systemprompt_identifiers::AiRequestId;

use super::extract::RejectionPartial;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub async fn persist_rejection(
    repos: &crate::services::gateway::GatewayRepositories,
    ai_request_id: &AiRequestId,
    partial: &RejectionPartial,
    status: StatusCode,
    message: &str,
) {
    let Some(record) = build_rejection_record(ai_request_id, partial) else {
        return;
    };
    write_rejection_record(&repos.requests, ai_request_id, &record, status, message).await;

    if let Some(body) = partial.body.as_ref() {
        write_rejection_payload(&repos.payloads, ai_request_id, body).await;
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn build_rejection_record(
    ai_request_id: &AiRequestId,
    partial: &RejectionPartial,
) -> Option<AiRequestRecord> {
    let Some(user_id) = partial.user_id.clone() else {
        tracing::debug!(
            ai_request_id = %ai_request_id,
            "No rejection record: request was rejected before it authenticated"
        );
        return None;
    };

    let mut builder = AiRequestRecord::builder(ai_request_id.clone(), user_id)
        .streaming(partial.is_streaming)
        .rejected();
    if let Some(provider) = &partial.provider {
        builder = builder.provider(provider.clone());
    }
    if let Some(model) = &partial.model {
        builder = builder.model(model.clone());
    }
    if let Some(s) = &partial.session_id {
        builder = builder.session_id(s.clone());
    }
    if let Some(c) = &partial.context_id {
        builder = builder.context_id(c.clone());
    }
    if let Some(t) = &partial.trace_id {
        builder = builder.trace_id(t.clone());
    }
    if let Some(mt) = partial.max_tokens {
        builder = builder.max_tokens(mt);
    }
    Some(builder.build())
}

async fn write_rejection_record(
    repo: &AiRequestRepository,
    ai_request_id: &AiRequestId,
    record: &AiRequestRecord,
    status: StatusCode,
    message: &str,
) {
    if let Err(e) = repo.insert_with_id(ai_request_id, record).await {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: insert failed");
        return;
    }
    let err_msg = format!("HTTP {}: {message}", status.as_u16());
    if let Err(e) = repo
        .update_error(ai_request_id, RequestStatus::Rejected, &err_msg)
        .await
    {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: update_error failed");
    }
}

async fn write_rejection_payload(
    payloads: &AiRequestPayloadRepository,
    ai_request_id: &AiRequestId,
    body: &Bytes,
) {
    let bytes_len = body.len().min(i32::MAX as usize) as i32;
    let sha256 = crate::services::gateway::audit::payload::digest_hex(body);
    let body_json = serde_json::from_slice::<serde_json::Value>(body).ok();
    let excerpt = if body_json.is_none() {
        Some(String::from_utf8_lossy(body).to_string())
    } else {
        None
    };
    if let Err(e) = payloads
        .upsert_request(
            ai_request_id,
            UpsertPayloadParams {
                body: body_json.as_ref(),
                excerpt: excerpt.as_deref(),
                truncated: false,
                bytes: Some(bytes_len),
                sha256: Some(sha256.as_str()),
            },
        )
        .await
    {
        tracing::warn!(error = %e, ai_request_id = %ai_request_id, "rejection audit: payload insert failed");
    }
}
