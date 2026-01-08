use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug)]
pub struct ComponentContext<'a> {
    pub item: &'a Value,
    pub all_items: &'a [Value],
    pub popular_ids: &'a [String],
    pub web_config: &'a serde_yaml::Value,
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

    async fn render(&self, ctx: &ComponentContext<'_>) -> Result<RenderedComponent>;

    fn priority(&self) -> u32 {
        100
    }
}
