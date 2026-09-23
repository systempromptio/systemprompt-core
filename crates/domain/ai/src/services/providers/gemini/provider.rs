//! Gemini provider client construction and request plumbing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::Result;
use crate::services::upstream::UpstreamTarget;
use reqwest::Client;
use systemprompt_database::DbPool;
use systemprompt_models::services::ProviderModel;
use systemprompt_models::services::providers::upstream_model_in;


use super::transport;

#[derive(Debug)]
pub struct GeminiProvider {
    pub(crate) client: Client,
    pub(crate) target: UpstreamTarget,
    pub(crate) db_pool: Option<DbPool>,
    pub(crate) google_search_enabled: bool,
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) default_model_override: Option<String>,
}

impl GeminiProvider {
    pub fn with_target(target: UpstreamTarget) -> Result<Self> {
        Ok(Self {
            client: transport::build_client()?,
            target,
            db_pool: None,
            google_search_enabled: false,
            models: Vec::new(),
            default_model_override: None,
        })
    }

    pub(crate) fn upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        upstream_model_in(&self.models, requested)
    }

    pub fn with_db_pool(mut self, db_pool: DbPool) -> Self {
        self.db_pool = Some(db_pool);
        self
    }

    pub const fn with_google_search(mut self) -> Self {
        self.google_search_enabled = true;
        self
    }

    #[must_use]
    pub fn with_models(mut self, models: Vec<ProviderModel>) -> Self {
        self.models = models;
        self
    }

    #[must_use]
    pub fn with_default_model(mut self, model: Option<String>) -> Self {
        self.default_model_override = model;
        self
    }

    pub const fn has_google_search(&self) -> bool {
        self.google_search_enabled
    }

    pub async fn generate_with_code_execution(
        &self,
        messages: &[crate::models::ai::AiMessage],
        sampling: Option<&crate::models::ai::SamplingParams>,
        max_output_tokens: u32,
        model: &str,
    ) -> Result<super::code_execution::CodeExecutionResponse> {
        super::code_execution::generate_with_code_execution(
            self,
            messages,
            sampling,
            max_output_tokens,
            model,
        )
        .await
    }
}
