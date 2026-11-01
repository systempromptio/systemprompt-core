//! [`ModelExtractor`]: core's extractor for every kind declaring a model
//! extractor. It sends the kind's versioned prompt and strict schema
//! through the injected [`BrainModelClient`] under the profile's timeout
//! and keeps the raw answer and its metering for the extraction ledger.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_extension::brain::{
    BrainExtractError, BrainModelClient, BrainModelError, FactExtractor,
};
use systemprompt_manifest::profile::BrainExtractionProfile;
use systemprompt_models::brain::{
    ExtractionInput, ExtractionOutput, ExtractionRecord, ExtractorDecl, KindDecl,
};

use crate::model::{ModelOutputError, parse_output, request_for};

/// Core's model-backed extractor.
#[derive(Clone)]
pub struct ModelExtractor {
    client: Arc<dyn BrainModelClient>,
    max_output_tokens: u32,
    timeout: Duration,
}

impl std::fmt::Debug for ModelExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelExtractor")
            .field("max_output_tokens", &self.max_output_tokens)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl ModelExtractor {
    pub const NAME: &'static str = "core.model";

    #[must_use]
    pub fn new(client: Arc<dyn BrainModelClient>, profile: &BrainExtractionProfile) -> Self {
        Self {
            client,
            max_output_tokens: profile.max_output_tokens,
            timeout: Duration::from_millis(profile.timeout_ms),
        }
    }
}

#[async_trait]
impl FactExtractor for ModelExtractor {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn handles(&self, kind: &KindDecl) -> bool {
        matches!(
            &kind.extractor,
            Some(ExtractorDecl::Model {
                prompt_version: Some(_),
                ..
            })
        )
    }

    async fn extract(
        &self,
        input: &ExtractionInput,
    ) -> Result<ExtractionOutput, BrainExtractError> {
        let kind = &input.kind;
        let Some(text) = &input.text else {
            return Err(failed(ModelOutputError::Schema(
                "no source text".to_owned(),
            )));
        };
        let hints = vec![kind.subject.clone()];
        let request = request_for(kind, text, &hints, self.max_output_tokens)
            .ok_or_else(|| failed(ModelOutputError::NoModelExtractor(kind.kind.clone())))?;
        let prompt_version = request.prompt_version.clone();
        let response = tokio::time::timeout(self.timeout, self.client.complete(request))
            .await
            .map_err(|_elapsed| BrainModelError::Timeout)??;
        let candidates =
            parse_output(kind, &response, &input.source, Self::NAME).map_err(failed)?;
        let raw_output = response
            .structured
            .clone()
            .unwrap_or_else(|| serde_json::Value::String(response.text.clone()));
        Ok(ExtractionOutput {
            candidates,
            record: ExtractionRecord {
                extractor: Self::NAME.to_owned(),
                model: Some(response.model),
                prompt_version: Some(prompt_version),
                raw_output,
                input_tokens: response.input_tokens,
                output_tokens: response.output_tokens,
                ai_request_id: response.ai_request_id,
            },
        })
    }
}

fn failed(error: ModelOutputError) -> BrainExtractError {
    BrainExtractError::Other {
        extractor: ModelExtractor::NAME,
        source: Box::new(error),
    }
}
