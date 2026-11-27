//! The engine's [`Governor`] contract over the applied instance: gateway-pinned
//! model calls, token renewal on refusal, usage counting, tool verdicts and the
//! instance's extension enablement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{TimeDelta, Utc};
use harness_engine::extension::EnabledExtensions;
use harness_engine::{
    Endpoint, GovernError, Governor, McpSpec, ModelCall, ModelSpec, Purpose, ResponseAction,
    ToolCall, Usage, Verdict,
};
use serde_json::json;
use sp_client::Freshness;
use sp_protocol::HOST_ID;
use sp_protocol::names::session_uuid;
use sp_protocol::wire::headers;
use sp_protocol::wire::hooks::session_metadata_user_id;

use super::Governance;
use crate::error::GovernanceError;
use crate::instance::{Instance, JWT_MIN_VALIDITY};
use crate::{hooks, telemetry};

const RENEW_FAILED: &str =
    "The instance refused the session token and a new one could not be had. Sign in again.";

const REFUSED_TWICE: &str =
    "The instance refused a freshly issued session token. Sign in to the instance again.";

fn unavailable(error: GovernanceError) -> GovernError {
    GovernError::Unavailable {
        source: Box::new(error),
    }
}

fn signed_endpoint(instance: &Instance, call: &ModelCall, bearer: &str) -> Endpoint {
    let mut endpoint = Endpoint::new(instance.base_url.clone());
    endpoint
        .headers
        .push(("authorization".to_owned(), format!("Bearer {bearer}")));
    endpoint
        .headers
        .push((headers::CLIENT.to_owned(), HOST_ID.to_owned()));
    if let Some(protocols) = &instance.protocols {
        endpoint
            .headers
            .push((headers::INFERENCE_PROTOCOL.to_owned(), protocols.clone()));
    }
    if instance.governance_plugin().is_some() {
        let session = session_uuid(call.session_id.as_str());
        endpoint.metadata =
            Some(json!({ "metadata": { "user_id": session_metadata_user_id(&session) } }));
    }
    endpoint
}

#[async_trait::async_trait]
impl Governor for Governance {
    fn models(&self) -> Vec<ModelSpec> {
        self.current()
            .map(|instance| instance.models.clone())
            .unwrap_or_default()
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        self.current()
            .map(|instance| instance.mcp.clone())
            .unwrap_or_default()
    }

    async fn authorize(&self, call: &ModelCall) -> Result<Endpoint, GovernError> {
        let Some(instance) = self.current() else {
            return Err(GovernError::NotSignedIn);
        };
        if !instance.offers(&call.model_id) {
            return Err(GovernError::ModelNotOffered {
                model: call.model_id.clone(),
            });
        }
        self.remember_directory(call.session_id.as_str(), &call.directory);
        let freshness = if instance.take_stale() {
            Freshness::Fresh
        } else {
            Freshness::ValidFor(JWT_MIN_VALIDITY)
        };
        let jwt = instance
            .credentials
            .jwt(freshness)
            .await
            .map_err(|error| unavailable(GovernanceError::Credential(error)))?;
        if jwt.is_expired(Utc::now(), TimeDelta::zero()) {
            tracing::warn!(
                session = %call.session_id,
                "the session token handed out has expired; the call is not sent"
            );
            return Err(unavailable(GovernanceError::ExpiredToken));
        }
        if self.current().is_none() {
            return Err(GovernError::NotSignedIn);
        }
        Ok(signed_endpoint(&instance, call, jwt.expose()))
    }

    async fn response(&self, call: &ModelCall, status: u16, attempt: u32) -> ResponseAction {
        let Some(instance) = self.current() else {
            return ResponseAction::Continue;
        };
        self.shared.usage.answered(status, Utc::now());
        let key = (call.session_id.as_str().to_owned(), call.purpose);
        if status != 401 {
            instance.forget_refusal(&key);
            return ResponseAction::Continue;
        }
        if instance.first_refusal(key.clone(), attempt) {
            tracing::info!(
                session = %call.session_id,
                attempt,
                "the gateway refused the session token; minting a fresh one"
            );
            return match instance.credentials.jwt(Freshness::Fresh).await {
                Ok(_fresh) => ResponseAction::Retry,
                Err(error) => {
                    instance.forget_refusal(&key);
                    tracing::warn!(
                        session = %call.session_id,
                        error = %error,
                        "no fresh session token after a refusal"
                    );
                    ResponseAction::Fail(RENEW_FAILED.to_owned())
                },
            };
        }
        instance.forget_refusal(&key);
        tracing::warn!(
            session = %call.session_id,
            attempt,
            "the gateway refused a freshly minted session token"
        );
        instance.credentials.rejected().await;
        instance.mark_stale();
        ResponseAction::Fail(REFUSED_TWICE.to_owned())
    }

    async fn step_finished(&self, call: &ModelCall, usage: &Usage) {
        if call.purpose == Purpose::Step && self.current().is_some() {
            self.shared.usage.step(usage);
        }
    }

    async fn tool_before(&self, call: &ToolCall) -> Verdict {
        if !self.verdicts_enabled() {
            return Verdict::Allow;
        }
        let Some(instance) = self.current() else {
            return Verdict::Allow;
        };
        let Some(plugin) = instance.governance_plugin().cloned() else {
            return Verdict::Allow;
        };
        telemetry::verdict(&instance, &plugin, &hooks::pre_tool(call)).await
    }

    fn extensions(&self) -> Option<EnabledExtensions> {
        self.current()?.extensions.clone()
    }

    fn generation(&self) -> u64 {
        self.shared.generation()
    }

    fn governed_by(&self) -> Option<String> {
        let instance = self.current()?;
        Some(
            instance
                .organisation
                .clone()
                .unwrap_or_else(|| instance.label.clone()),
        )
    }
}
