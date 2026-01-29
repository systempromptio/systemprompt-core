use std::path::PathBuf;

use crate::web_config::WebConfig;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum PartialSource {
    Embedded(&'static str),
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct PartialTemplate {
    pub name: String,
    pub source: PartialSource,
}

impl PartialTemplate {
    #[must_use]
    pub fn embedded(name: impl Into<String>, content: &'static str) -> Self {
        Self {
            name: name.into(),
            source: PartialSource::Embedded(content),
        }
    }

    #[must_use]
    pub fn file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: PartialSource::File(path.into()),
        }
    }
}

#[derive(Debug)]
pub struct ComponentContext<'a> {
    pub web_config: &'a WebConfig,
    pub item: Option<&'a Value>,
    pub all_items: Option<&'a [Value]>,
    pub popular_ids: Option<&'a [String]>,
}

impl<'a> ComponentContext<'a> {
    #[must_use]
    pub const fn for_page(web_config: &'a WebConfig) -> Self {
        Self {
            web_config,
            item: None,
            all_items: None,
            popular_ids: None,
        }
    }

    #[must_use]
    pub const fn for_content(
        web_config: &'a WebConfig,
        item: &'a Value,
        all_items: &'a [Value],
        popular_ids: &'a [String],
    ) -> Self {
        Self {
            web_config,
            item: Some(item),
            all_items: Some(all_items),
            popular_ids: Some(popular_ids),
        }
    }

    #[must_use]
    pub const fn for_list(web_config: &'a WebConfig, all_items: &'a [Value]) -> Self {
        Self {
            web_config,
            item: None,
            all_items: Some(all_items),
            popular_ids: None,
        }
    }
}

#[derive(Debug)]
pub struct RenderedComponent {
    pub html: String,
    pub variable_name: String,
}

impl RenderedComponent {
    #[must_use]
    pub fn new(variable_name: impl Into<String>, html: impl Into<String>) -> Self {
        Self {
            html: html.into(),
            variable_name: variable_name.into(),
        }
    }
}

#[async_trait]
pub trait ComponentRenderer: Send + Sync {
    fn component_id(&self) -> &str;

    fn variable_name(&self) -> &str;

    fn applies_to(&self) -> Vec<String> {
        vec![]
    }

    fn partial_template(&self) -> Option<PartialTemplate> {
        None
    }

    async fn render(&self, ctx: &ComponentContext<'_>) -> Result<RenderedComponent>;

    fn priority(&self) -> u32 {
        100
    }
}
