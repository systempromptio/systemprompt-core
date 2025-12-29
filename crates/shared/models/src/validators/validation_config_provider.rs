//! Config provider wrapper for validation with all pre-loaded configs.

use crate::{Config, ContentConfigRaw, ServicesConfig};
use systemprompt_traits::ConfigProvider;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WebConfigRaw {
    #[serde(default)]
    pub site_name: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WebMetadataRaw {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug)]
pub struct ValidationConfigProvider {
    config: Config,
    services_config: ServicesConfig,
    content_config: Option<ContentConfigRaw>,
    web_config: Option<WebConfigRaw>,
    web_metadata: Option<WebMetadataRaw>,
}

impl ValidationConfigProvider {
    pub const fn new(config: Config, services_config: ServicesConfig) -> Self {
        Self {
            config,
            services_config,
            content_config: None,
            web_config: None,
            web_metadata: None,
        }
    }

    pub fn with_content_config(mut self, config: ContentConfigRaw) -> Self {
        self.content_config = Some(config);
        self
    }

    pub fn with_web_config(mut self, config: WebConfigRaw) -> Self {
        self.web_config = Some(config);
        self
    }

    pub fn with_web_metadata(mut self, metadata: WebMetadataRaw) -> Self {
        self.web_metadata = Some(metadata);
        self
    }

    pub const fn services_config(&self) -> &ServicesConfig {
        &self.services_config
    }

    pub const fn config(&self) -> &Config {
        &self.config
    }

    pub const fn content_config(&self) -> Option<&ContentConfigRaw> {
        self.content_config.as_ref()
    }

    pub const fn web_config(&self) -> Option<&WebConfigRaw> {
        self.web_config.as_ref()
    }

    pub const fn web_metadata(&self) -> Option<&WebMetadataRaw> {
        self.web_metadata.as_ref()
    }
}

impl ConfigProvider for ValidationConfigProvider {
    fn get(&self, key: &str) -> Option<String> {
        self.config.get(key)
    }

    fn database_url(&self) -> &str {
        self.config.database_url()
    }

    fn system_path(&self) -> &str {
        self.config.system_path()
    }

    fn api_port(&self) -> u16 {
        self.config.api_port()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
