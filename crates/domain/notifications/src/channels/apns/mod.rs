//! The Apple push channel.
//!
//! A push is a background notification: `aps` carries only
//! `content-available`, beside the three [`PushPayload`] fields. There is no
//! alert, title, body or localisation key, so nothing the app displays comes
//! from the provider; the app wakes, shows its own line for the category and
//! fetches the notification. The collapse key is sent as `apns-collapse-id`.
//!
//! Only a token issued to the declared topic (the app's bundle id) is pushed
//! to. Apple's `410` and `Unregistered` answers, and a stored token that is
//! not hexadecimal, mark the token unregistered. `BadDeviceToken` does not:
//! Apple sends it for a token from the other environment too.
//! Throttling, a server failure and an expired provider token are worth
//! another attempt; every other refusal is not.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod signer;

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use systemprompt_extension::notifications::DevicePushToken;
use systemprompt_models::devices::AppIdentifier;
use systemprompt_models::notifications::{PushPayload, PushProvider};

pub use signer::ApnsSigner;

use super::push::{
    ProviderCredential, ProviderHttp, ProviderRequest, ProviderResponse, PushChannel, PushError,
    PushSender, PushVerdict,
};
use crate::plane::push::ApnsEnvironment;

// Why: Apple requires priority 5 for a push of type `background` and refuses
// one sent at 10.
const BACKGROUND_PRIORITY: &str = "5";
const BACKGROUND_PUSH_TYPE: &str = "background";

/// The channel that pushes through Apple's push service.
pub type ApnsChannel = PushChannel<ApnsSender>;

#[derive(Serialize)]
struct Aps {
    #[serde(rename = "content-available")]
    content_available: u8,
}

#[derive(Serialize)]
struct Body<'a> {
    aps: Aps,
    #[serde(flatten)]
    payload: &'a PushPayload,
}

#[derive(Deserialize)]
struct Failure {
    #[serde(default)]
    reason: String,
}

/// Sends one payload to Apple's push service for one device token.
pub struct ApnsSender {
    http: Arc<dyn ProviderHttp>,
    credential: Arc<dyn ProviderCredential>,
    environment: ApnsEnvironment,
    topic: AppIdentifier,
}

impl ApnsSender {
    #[must_use]
    pub fn new(
        http: Arc<dyn ProviderHttp>,
        credential: Arc<dyn ProviderCredential>,
        environment: ApnsEnvironment,
        topic: AppIdentifier,
    ) -> Self {
        Self {
            http,
            credential,
            environment,
            topic,
        }
    }
}

impl fmt::Debug for ApnsSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApnsSender")
            .field("environment", &self.environment)
            .field("topic", &self.topic)
            .finish_non_exhaustive()
    }
}

fn verdict(response: &ProviderResponse) -> Result<PushVerdict, PushError> {
    let status = response.status;
    if status == 200 {
        return Ok(PushVerdict::Accepted);
    }
    let reason = serde_json::from_str::<Failure>(&response.body)
        .map(|failure| failure.reason)
        .unwrap_or_default();
    match (status, reason.as_str()) {
        // Why: Apple also answers BadDeviceToken when a sandbox token meets
        // the production host (or the reverse), so it is a rejection: treating
        // it as unregistered would delete every token of a misconfigured
        // instance on first delivery.
        (410, _) | (_, "Unregistered") => Ok(PushVerdict::Unregistered),
        (429 | 500..=599, _) | (_, "ExpiredProviderToken") => {
            Err(PushError::Unavailable { status, reason })
        },
        _ => Err(PushError::Rejected { status, reason }),
    }
}

impl PushSender for ApnsSender {
    fn provider(&self) -> PushProvider {
        PushProvider::Apns
    }

    fn serves(&self, token: &DevicePushToken) -> bool {
        token.provider == PushProvider::Apns && token.app_id == self.topic
    }

    async fn send(
        &self,
        token: &DevicePushToken,
        payload: &PushPayload,
    ) -> Result<PushVerdict, PushError> {
        let device = token.token.expose();
        if !device.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(PushVerdict::Unregistered);
        }
        let body = serde_json::to_string(&Body {
            aps: Aps {
                content_available: 1,
            },
            payload,
        })?;
        let request = ProviderRequest {
            url: format!("{}/3/device/{device}", self.environment.origin()),
            bearer: self.credential.bearer().await?,
            headers: vec![
                ("apns-topic", self.topic.as_str().to_owned()),
                ("apns-push-type", BACKGROUND_PUSH_TYPE.to_owned()),
                ("apns-priority", BACKGROUND_PRIORITY.to_owned()),
                ("apns-collapse-id", payload.collapse_key.as_str().to_owned()),
            ],
            body,
        };
        verdict(&self.http.post(request).await?)
    }
}
