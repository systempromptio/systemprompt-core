//! [`ComponentRenderer`] contract for emitting one named template component.

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ProviderResult;
use crate::web_config::WebConfig;

/// Source of a partial template referenced by a [`ComponentRenderer`].
#[derive(Debug, Clone)]
pub enum PartialSource {
    /// Source embedded in the binary at compile time.
    Embedded(&'static str),
    /// Source loaded from disk at runtime.
    File(PathBuf),
}

/// Named partial template registered alongside a [`ComponentRenderer`].
#[derive(Debug, Clone)]
pub struct PartialTemplate {
    /// Template name as referenced from other templates.
    pub name: String,
    /// Source of the partial body.
    pub source: PartialSource,
}

impl PartialTemplate {
    /// Build a [`PartialTemplate`] backed by an embedded `&'static str`.
    #[must_use]
    pub fn embedded(name: impl Into<String>, content: &'static str) -> Self {
        Self {
            name: name.into(),
            source: PartialSource::Embedded(content),
        }
    }

    /// Build a [`PartialTemplate`] backed by a file path.
    #[must_use]
    pub fn file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: PartialSource::File(path.into()),
        }
    }
}

/// Per-call context handed to a [`ComponentRenderer`].
#[derive(Debug)]
pub struct ComponentContext<'a> {
    /// Resolved web config for the rendering host.
    pub web_config: &'a WebConfig,
    /// Active content item, when rendering inside one.
    pub item: Option<&'a Value>,
    /// Full item list, when rendering inside a list page.
    pub all_items: Option<&'a [Value]>,
    /// Optional list of "popular" content ids for sidebar widgets.
    pub popular_ids: Option<&'a [String]>,
}

impl<'a> ComponentContext<'a> {
    /// Build a [`ComponentContext`] for a generic page (no item context).
    #[must_use]
    pub const fn for_page(web_config: &'a WebConfig) -> Self {
        Self {
            web_config,
            item: None,
            all_items: None,
            popular_ids: None,
        }
    }

    /// Build a [`ComponentContext`] for a single content item.
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

    /// Build a [`ComponentContext`] for a list page.
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

/// HTML emitted by a [`ComponentRenderer`].
#[derive(Debug)]
pub struct RenderedComponent {
    /// Rendered HTML.
    pub html: String,
    /// Template variable name [`Self::html`] should be bound to.
    pub variable_name: String,
}

impl RenderedComponent {
    /// Build a [`RenderedComponent`] from its parts.
    #[must_use]
    pub fn new(variable_name: impl Into<String>, html: impl Into<String>) -> Self {
        Self {
            html: html.into(),
            variable_name: variable_name.into(),
        }
    }
}

/// Hook that renders a named component into a template variable.
///
/// Marked `#[async_trait]` because it is consumed via `dyn ComponentRenderer`.
#[async_trait]
pub trait ComponentRenderer: Send + Sync {
    /// Stable identifier for this component.
    fn component_id(&self) -> &'static str;

    /// Template variable name produced by this component.
    fn variable_name(&self) -> &'static str;

    /// Page-type names this component opts into; empty means "all".
    fn applies_to(&self) -> Vec<String> {
        vec![]
    }

    /// Optional partial template registered alongside this component.
    fn partial_template(&self) -> Option<PartialTemplate> {
        None
    }

    /// Render the component body into HTML.
    async fn render(&self, ctx: &ComponentContext<'_>) -> ProviderResult<RenderedComponent>;

    /// Component priority; higher runs first.
    fn priority(&self) -> u32 {
        100
    }
}
