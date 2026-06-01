use crate::error::Result;
use reqwest::Client;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::profile::ProviderModel;
use tokio::sync::Mutex;

use crate::services::schema::ToolNameMapper;

use super::constants::defaults;
use super::transport;

#[derive(Debug)]
pub struct GeminiProvider {
    pub(crate) client: Client,
    pub(crate) api_key: String,
    pub(crate) endpoint: String,
    pub(crate) tool_mapper: Arc<Mutex<ToolNameMapper>>,
    pub(crate) db_pool: Option<DbPool>,
    pub(crate) google_search_enabled: bool,
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) default_model_override: Option<String>,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Result<Self> {
        let client = transport::build_client()?;
        Ok(Self {
            client,
            api_key,
            endpoint: defaults::ENDPOINT.to_owned(),
            tool_mapper: Arc::new(Mutex::new(ToolNameMapper::new())),
            db_pool: None,
            google_search_enabled: false,
            models: Vec::new(),
            default_model_override: None,
        })
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Result<Self> {
        let client = transport::build_client()?;
        Ok(Self {
            client,
            api_key,
            endpoint,
            tool_mapper: Arc::new(Mutex::new(ToolNameMapper::new())),
            db_pool: None,
            google_search_enabled: false,
            models: Vec::new(),
            default_model_override: None,
        })
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
