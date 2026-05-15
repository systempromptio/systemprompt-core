//! [`ComponentRenderer`] contract for emitting one named template component.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ProviderResult;
use crate::web_config::WebConfig;

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

// Why: renderer is consumed as a trait object by the generator crate; an
// async fn in a bare trait is not dyn-compatible, so #[async_trait] is
// required.
#[async_trait]
pub trait ComponentRenderer: Send + Sync {
    fn component_id(&self) -> &'static str;

    fn variable_name(&self) -> &'static str;

    fn applies_to(&self) -> Vec<String> {
        vec![]
    }

    fn partial_template(&self) -> Option<PartialTemplate> {
        None
    }

    async fn render(&self, ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent>;

    fn priority(&self) -> u32 {
        100
    }
}
