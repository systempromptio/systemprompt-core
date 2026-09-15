//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::credentials::Enrollment;
use super::{FeedbackError, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DeviceId, UserId};
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptRequest, ConsumerReceiptResponse,
    SessionBindingRequest,
};

#[derive(Debug, Deserialize)]
pub struct EnrollmentResponse {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(FeedbackError::from)
}

pub async fn enroll(gateway: &str, credential: &str) -> Result<EnrollmentResponse> {
    if !credential.starts_with("sp_device_") || credential.len() > 256 {
        return Err(FeedbackError::EnrollmentRequired);
    }
    let gateway = super::credentials::GatewayOrigin::parse(gateway)?;
    decode(
        client()?
            .post(format!("{gateway}/api/v1/consumer-devices/enrollment"))
            .bearer_auth(credential)
            .send()
            .await?,
        4096,
    )
    .await
}

pub async fn receipt(
    enrollment: &Enrollment,
    request: &ConsumerReceiptRequest,
) -> Result<ConsumerReceiptResponse> {
    post(enrollment, "/api/v1/consumer/receipts", request).await
}

pub async fn bind(enrollment: &Enrollment, request: &SessionBindingRequest) -> Result<()> {
    // JSON: protocol boundary — the binding acknowledgement body is not consumed.
    let _: serde_json::Value =
        post(enrollment, "/api/v1/consumer/session-bindings", request).await?;
    Ok(())
}

pub async fn plan(
    enrollment: &Enrollment,
    publication: &systemprompt_models::bridge::manifest::SkillPublication,
    requested_host: systemprompt_models::feedback::EvaluatorClient,
) -> Result<ConsumerInstallationPlan> {
    let resource: String =
        url::form_urlencoded::byte_serialize(publication.resource_id.as_str().as_bytes()).collect();
    let publication_id: String =
        url::form_urlencoded::byte_serialize(publication.publication_id.as_str().as_bytes())
            .collect();
    let host = serde_json::to_value(requested_host)?
        .as_str()
        .ok_or(FeedbackError::Scope)?
        .to_owned();
    let url = format!(
        "{}/api/v1/consumer/resources/{resource}/publications/{publication_id}/bundle?host={host}",
        enrollment.gateway
    );
    let plan: ConsumerInstallationPlan = decode(
        client()?
            .get(url)
            .bearer_auth(enrollment.credential())
            .send()
            .await?,
        64 * 1024 * 1024,
    )
    .await?;
    if plan.host != requested_host
        || plan.publication_id != publication.publication_id
        || plan.resource_id != publication.resource_id
        || plan.revision_id != publication.revision_id
        || plan.generation != publication.generation
        || plan.bundle_digest.as_str() != publication.bundle_digest.as_str()
    {
        return Err(FeedbackError::Readback);
    }
    Ok(plan)
}

async fn post<T: Serialize + Sync, R: DeserializeOwned>(
    enrollment: &Enrollment,
    path: &str,
    body: &T,
) -> Result<R> {
    decode(
        client()?
            .post(format!("{}{path}", enrollment.gateway))
            .bearer_auth(enrollment.credential())
            .json(body)
            .send()
            .await?,
        1024 * 1024,
    )
    .await
}

async fn decode<T: DeserializeOwned>(mut response: reqwest::Response, maximum: usize) -> Result<T> {
    if !response.status().is_success() {
        return Err(FeedbackError::Rejected(response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|size| size > maximum as u64)
    {
        return Err(FeedbackError::Transport);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err(FeedbackError::Transport);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&bytes)?)
}
