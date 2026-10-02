//! [`PagePrerenderer`] contract for emitting one statically-rendered page.
//!
//! Prerenderers are held as [`DynPagePrerenderer`] (`Arc<dyn
//! PagePrerenderer>`), so the trait uses `#[async_trait]`; native `async fn` in
//! traits is not `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use systemprompt_identifiers::LocaleCode;

use crate::dependencies::{Dependencies, MissingDependency};
use crate::error::ProviderResult;
use crate::web_config::WebConfig;

#[derive(Debug)]
pub struct PagePrepareContext<'a> {
    pub web_config: &'a WebConfig,
    pub locale: &'a LocaleCode,
    dependencies: &'a Dependencies,
    dist_dir: &'a std::path::Path,
}

impl<'a> PagePrepareContext<'a> {
    #[must_use]
    pub const fn new(
        web_config: &'a WebConfig,
        dependencies: &'a Dependencies,
        dist_dir: &'a std::path::Path,
    ) -> Self {
        Self {
            web_config,
            locale: &web_config.i18n.default_locale,
            dependencies,
            dist_dir,
        }
    }

    #[must_use]
    pub const fn with_locale(mut self, locale: &'a LocaleCode) -> Self {
        self.locale = locale;
        self
    }

    pub fn get<T: Any + Send + Sync>(&self) -> Result<&T, MissingDependency> {
        self.dependencies.get::<T>()
    }

    #[must_use]
    pub const fn dist_dir(&self) -> &std::path::Path {
        self.dist_dir
    }
}

#[derive(Debug, Clone)]
pub struct PageRenderSpec {
    pub template_name: String,
    // JSON: Handlebars template base context; the page data model is dynamic.
    pub base_data: Value,
    pub output_path: PathBuf,
}

impl PageRenderSpec {
    #[must_use]
    pub fn new(
        template_name: impl Into<String>,
        // JSON: Handlebars template base context; the page data model is dynamic.
        base_data: Value,
        output_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            template_name: template_name.into(),
            base_data,
            output_path: output_path.into(),
        }
    }
}

pub type DynPagePrerenderer = std::sync::Arc<dyn PagePrerenderer>;

#[async_trait]
pub trait PagePrerenderer: Send + Sync {
    fn page_type(&self) -> &str;

    fn priority(&self) -> u32 {
        100
    }

    async fn prepare(&self, ctx: &PagePrepareContext<'_>)
    -> ProviderResult<Option<PageRenderSpec>>;
}
