//! The Google push channel.
//!
//! A push is a data message: `data` is exactly the three [`PushPayload`]
//! fields and there is no `notification` block, so nothing the app displays
//! comes from the provider. The collapse key is also sent as the Android
//! collapse key, and the message is sent at high priority because only a
//! notification at human attention is pushed.
//!
//! Google's `404`, `NOT_FOUND` and `UNREGISTERED` answers mark the token
//! unregistered. Throttling, a server failure and a refused access token are
//! worth another attempt; every other refusal is not.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access;

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use systemprompt_extension::notifications::DevicePushToken;
use systemprompt_models::notifications::{PushPayload, PushProvider};

pub use access::{FcmAccess, FcmProjectId, FcmProjectIdError};

use super::push::{
    ProviderCredential, ProviderHttp, ProviderRequest, ProviderResponse, PushChannel, PushError,
    PushSender, PushVerdict,
};

const FCM_ORIGIN: &str = "https://fcm.googleapis.com";
const HIGH_PRIORITY: &str = "HIGH";

/// The channel that pushes through Google's push service.
pub type FcmChannel = PushChannel<FcmSender>;

#[derive(Serialize)]
struct Android<'a> {
    collapse_key: &'a str,
    priority: &'static str,
}

#[derive(Serialize)]
struct Message<'a> {
    token: &'a str,
    data: &'a PushPayload,
    android: Android<'a>,
}

#[derive(Serialize)]
struct Body<'a> {
    message: Message<'a>,
}

#[derive(Deserialize)]
struct Failure {
    error: FailureBody,
}

#[derive(Deserialize)]
struct FailureBody {
    #[serde(default)]
    status: String,
    #[serde(default)]
    details: Vec<FailureDetail>,
}

#[derive(Deserialize)]
struct FailureDetail {
    #[serde(default, rename = "errorCode")]
    error_code: Option<String>,
}

/// Sends one payload to Google's push service for one device token.
pub struct FcmSender {
    http: Arc<dyn ProviderHttp>,
    credential: Arc<dyn ProviderCredential>,
    project: FcmProjectId,
}

impl FcmSender {
    #[must_use]
    pub fn new(
        http: Arc<dyn ProviderHttp>,
        credential: Arc<dyn ProviderCredential>,
        project: FcmProjectId,
    ) -> Self {
        Self {
            http,
            credential,
            project,
        }
    }
}

impl fmt::Debug for FcmSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FcmSender")
            .field("project", &self.project)
            .finish_non_exhaustive()
    }
}

fn reason_of(body: &str) -> String {
    serde_json::from_str::<Failure>(body)
        .map(|failure| {
            failure
                .error
                .details
                .into_iter()
                .find_map(|detail| detail.error_code)
                .unwrap_or(failure.error.status)
        })
        .unwrap_or_default()
}

fn verdict(response: &ProviderResponse) -> Result<PushVerdict, PushError> {
    let status = response.status;
    if (200..300).contains(&status) {
        return Ok(PushVerdict::Accepted);
    }
    let reason = reason_of(&response.body);
    match (status, reason.as_str()) {
        (404, _) | (_, "UNREGISTERED" | "NOT_FOUND") => Ok(PushVerdict::Unregistered),
        (401 | 429 | 500..=599, _) => Err(PushError::Unavailable { status, reason }),
        _ => Err(PushError::Rejected { status, reason }),
    }
}

impl PushSender for FcmSender {
    fn provider(&self) -> PushProvider {
        PushProvider::Fcm
    }

    async fn send(
        &self,
        token: &DevicePushToken,
        payload: &PushPayload,
    ) -> Result<PushVerdict, PushError> {
        let body = serde_json::to_string(&Body {
            message: Message {
                token: token.token.expose(),
                data: payload,
                android: Android {
                    collapse_key: payload.collapse_key.as_str(),
                    priority: HIGH_PRIORITY,
                },
            },
        })?;
        let request = ProviderRequest {
            url: format!(
                "{FCM_ORIGIN}/v1/projects/{}/messages:send",
                self.project.as_str()
            ),
            bearer: self.credential.bearer().await?,
            headers: Vec::new(),
            body,
        };
        verdict(&self.http.post(request).await?)
    }
}
