use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug)]
pub struct ExtenderContext<'a> {
    pub item: &'a Value,
    pub all_items: &'a [Value],
    pub config: &'a serde_yaml::Value,
    pub web_config: &'a serde_yaml::Value,
    pub content_html: &'a str,
    pub url_pattern: &'a str,
    pub source_name: &'a str,
}

#[derive(Debug)]
pub struct ExtendedData {
    pub variables: Value,
    pub priority: u32,
}

impl ExtendedData {
    #[must_use]
    pub const fn new(variables: Value) -> Self {
        Self {
            variables,
            priority: 100,
        }
    }

    #[must_use]
    pub const fn with_priority(variables: Value, priority: u32) -> Self {
        Self { variables, priority }
    }
}

#[async_trait]
pub trait TemplateDataExtender: Send + Sync {
    fn extender_id(&self) -> &str;

    fn applies_to(&self) -> Vec<String> {
        vec![]
    }

    async fn extend(&self, ctx: &ExtenderContext<'_>, data: &mut Value) -> Result<()>;

    fn priority(&self) -> u32 {
        100
    }
}
