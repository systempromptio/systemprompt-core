//! The brain's extraction model calls, routed through the gateway pipeline.
//!
//! [`GatewayClient`] implements the shared [`BrainModelClient`] seam, so the
//! brain consumer holds only `Arc<dyn BrainModelClient>` and never links a
//! provider or the gateway. Each call is a single user turn with the prompt's
//! system text, a strict JSON Schema response format and the request's output
//! cap, attributed `purpose=brain.extract`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::StatusCode;
use systemprompt_extension::brain::{BrainModelClient, BrainModelError};
use systemprompt_models::brain::{BrainModelRequest, BrainModelResponse};

use super::GatewayClient;
use super::error::{DecodeError, GatewayClientError};
use crate::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, ResponseFormat, Role,
};

pub const BRAIN_EXTRACT_PURPOSE: &str = "brain.extract";

#[async_trait::async_trait]
impl BrainModelClient for GatewayClient {
    async fn complete(
        &self,
        request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError> {
        let model = self
            .default_model()
            .cloned()
            .ok_or(BrainModelError::Unavailable)?;
        let mut canonical = CanonicalRequest::new(
            model,
            vec![CanonicalMessage {
                role: Role::User,
                content: vec![CanonicalContent::text(request.user)],
            }],
            request.max_output_tokens,
        );
        canonical.set_system_text(Some(request.system));
        canonical.response_format = Some(ResponseFormat::JsonSchema {
            name: request.json_schema.name,
            schema: request.json_schema.schema,
            strict: true,
        });
        let completion = Self::complete(self, canonical, BRAIN_EXTRACT_PURPOSE)
            .await
            .map_err(brain_error)?;
        Ok(BrainModelResponse {
            model: completion.model,
            text: completion.text,
            structured: completion.structured,
            input_tokens: completion.usage.input_tokens,
            output_tokens: completion.usage.output_tokens,
            ai_request_id: Some(completion.ai_request_id),
        })
    }
}

fn brain_error(error: GatewayClientError) -> BrainModelError {
    match error {
        GatewayClientError::Timeout => BrainModelError::Timeout,
        GatewayClientError::Upstream { status, .. }
            if status == StatusCode::GATEWAY_TIMEOUT || status == StatusCode::REQUEST_TIMEOUT =>
        {
            BrainModelError::Timeout
        },
        GatewayClientError::Denied { reason, .. } => BrainModelError::Refused {
            reason: reason.message,
        },
        GatewayClientError::Quota { .. } => BrainModelError::Refused {
            reason: "brain service principal quota exhausted".to_owned(),
        },
        GatewayClientError::Decode(DecodeError::Structured(source)) => {
            BrainModelError::Decode(source)
        },
        other => BrainModelError::Other(Box::new(other)),
    }
}
