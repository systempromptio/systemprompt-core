//! The device-authenticated consumer API that delivers a skill's runtime files,
//! device enrolment, and the heartbeat.
//!
//! Enrolment and the heartbeat take the user's JWT and live under the instance
//! base URL. Plans, receipts and session bindings take the enrolled device
//! credential, live at the instance's origin, and are classified for the
//! caller's outbox: acknowledged, refused for good (`400`, `403`, `404`, `409`,
//! `422`), refused device (`401`: forget the device and enrol again) or retry
//! later (everything else, a plan that is not the publication named included).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use sp_protocol::wire::consumer::{
    DeviceEnrolRequest, DeviceEnrolment, InstallationPlan, ReceiptRequest, ReceiptResponse,
    SessionBinding, SessionBindingRequest,
};
use sp_protocol::wire::heartbeat::{HeartbeatRequest, HeartbeatResponse};
use sp_protocol::wire::manifest::Publication;
use sp_protocol::wire::paths;
use sp_protocol::{DeviceCredential, HOST_ID, Jwt};

use crate::client::{SpClient, push_segments};
use crate::error::ClientError;
use crate::http::{SMALL_BODY, decode, exchange, failure, protocol};

const ENROL_ENDPOINT: &str = "POST /v1/bridge/device";
const PLAN_ENDPOINT: &str =
    "GET /api/v1/consumer/resources/{resource}/publications/{publication}/bundle";
const RECEIPT_ENDPOINT: &str = "POST /api/v1/consumer/receipts";
const BINDING_ENDPOINT: &str = "POST /api/v1/consumer/session-bindings";
const HEARTBEAT_ENDPOINT: &str = "POST /v1/bridge/heartbeat";

const CONSUMER_TIMEOUT: Duration = Duration::from_secs(20);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);
const PLAN_BODY: usize = 64 * 1024 * 1024;
const RECEIPT_BODY: usize = 1024 * 1024;

const REFUSED_FOR_GOOD: [u16; 5] = [400, 403, 404, 409, 422];

/// How the instance answered a consumer call, as an outbox needs it.
#[derive(Debug)]
pub enum ConsumerOutcome<T> {
    Acknowledged(T),
    Refused(ClientError),
    DeviceRefused(ClientError),
    RetryLater(ClientError),
}

impl<T> ConsumerOutcome<T> {
    #[must_use]
    pub fn classify(result: Result<T, ClientError>) -> Self {
        match result {
            Ok(value) => Self::Acknowledged(value),
            Err(error) if error.is_unauthorized() => Self::DeviceRefused(error),
            Err(error)
                if error
                    .status()
                    .is_some_and(|status| REFUSED_FOR_GOOD.contains(&status)) =>
            {
                Self::Refused(error)
            },
            Err(error) => Self::RetryLater(error),
        }
    }

    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RetryLater(error) => error.retry_after(),
            Self::Acknowledged(_) | Self::Refused(_) | Self::DeviceRefused(_) => None,
        }
    }

    pub fn into_result(self) -> Result<T, ClientError> {
        match self {
            Self::Acknowledged(value) => Ok(value),
            Self::Refused(error) | Self::DeviceRefused(error) | Self::RetryLater(error) => {
                Err(error)
            },
        }
    }
}

impl SpClient {
    pub async fn enrol_device(
        &self,
        jwt: &Jwt,
        request: &DeviceEnrolRequest,
    ) -> Result<DeviceEnrolment, ClientError> {
        let builder = self
            .post(self.at(paths::DEVICE)?)
            .bearer_auth(jwt.expose())
            .json(request);
        let answer = exchange(ENROL_ENDPOINT, builder, CONSUMER_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(ENROL_ENDPOINT, &answer));
        }
        decode(ENROL_ENDPOINT, &answer)
    }

    pub async fn installation_plan(
        &self,
        device: &DeviceCredential,
        publication: &Publication,
    ) -> ConsumerOutcome<InstallationPlan> {
        ConsumerOutcome::classify(self.fetch_plan(device, publication).await)
    }

    pub async fn post_receipt(
        &self,
        device: &DeviceCredential,
        receipt: &ReceiptRequest,
    ) -> ConsumerOutcome<ReceiptResponse> {
        ConsumerOutcome::classify(
            self.consumer_post(RECEIPT_ENDPOINT, paths::CONSUMER_RECEIPTS, device, receipt)
                .await,
        )
    }

    pub async fn post_session_binding(
        &self,
        device: &DeviceCredential,
        binding: &SessionBindingRequest,
    ) -> ConsumerOutcome<SessionBinding> {
        ConsumerOutcome::classify(
            self.consumer_post(
                BINDING_ENDPOINT,
                paths::CONSUMER_SESSION_BINDINGS,
                device,
                binding,
            )
            .await,
        )
    }

    pub async fn heartbeat(
        &self,
        jwt: &Jwt,
        request: &HeartbeatRequest,
    ) -> Result<HeartbeatResponse, ClientError> {
        let builder = self
            .post(self.at(paths::HEARTBEAT)?)
            .bearer_auth(jwt.expose())
            .json(request);
        let answer = exchange(HEARTBEAT_ENDPOINT, builder, HEARTBEAT_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(HEARTBEAT_ENDPOINT, &answer));
        }
        decode(HEARTBEAT_ENDPOINT, &answer)
    }

    async fn fetch_plan(
        &self,
        device: &DeviceCredential,
        publication: &Publication,
    ) -> Result<InstallationPlan, ClientError> {
        let mut url = self.at_origin(paths::CONSUMER_RESOURCES)?;
        push_segments(
            &mut url,
            [
                publication.resource_id.as_str(),
                "publications",
                publication.publication_id.as_str(),
                "bundle",
            ],
        )?;
        url.query_pairs_mut().append_pair("host", HOST_ID);
        let request = self.get(url).bearer_auth(device.expose());
        let answer = exchange(PLAN_ENDPOINT, request, CONSUMER_TIMEOUT, PLAN_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(PLAN_ENDPOINT, &answer));
        }
        let plan: InstallationPlan = decode(PLAN_ENDPOINT, &answer)?;
        if !plan.matches(publication, HOST_ID) {
            return Err(protocol(
                PLAN_ENDPOINT,
                Some(200),
                "the plan is not the publication the manifest named",
            ));
        }
        Ok(plan)
    }

    async fn consumer_post<B, R>(
        &self,
        endpoint: &'static str,
        path: &str,
        device: &DeviceCredential,
        body: &B,
    ) -> Result<R, ClientError>
    where
        B: serde::Serialize + Sync + ?Sized,
        R: serde::de::DeserializeOwned,
    {
        let request = self
            .post(self.at_origin(path)?)
            .bearer_auth(device.expose())
            .header(CONTENT_TYPE, "application/json")
            .json(body);
        let answer = exchange(endpoint, request, CONSUMER_TIMEOUT, RECEIPT_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(endpoint, &answer));
        }
        decode(endpoint, &answer)
    }
}
