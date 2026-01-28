use std::any::Any;
use std::path::PathBuf;

use crate::web_config::WebConfig;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug)]
pub struct PagePrepareContext<'a> {
    pub web_config: &'a WebConfig,
    content_config: &'a (dyn Any + Send + Sync),
    db_pool: &'a (dyn Any + Send + Sync),
    dist_dir: &'a std::path::Path,
}

impl<'a> PagePrepareContext<'a> {
    #[must_use]
    pub fn new(
        web_config: &'a WebConfig,
        content_config: &'a (dyn Any + Send + Sync),
        db_pool: &'a (dyn Any + Send + Sync),
        dist_dir: &'a std::path::Path,
    ) -> Self {
        Self {
            web_config,
            content_config,
            db_pool,
            dist_dir,
        }
    }

    #[must_use]
    pub fn content_config<T: 'static>(&self) -> Option<&T> {
        self.content_config.downcast_ref::<T>()
    }

    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }

    #[must_use]
    pub const fn dist_dir(&self) -> &std::path::Path {
        self.dist_dir
    }
}

#[derive(Debug, Clone)]
pub struct PageRenderSpec {
    pub template_name: String,
    pub base_data: Value,
    pub output_path: PathBuf,
}

impl PageRenderSpec {
    #[must_use]
    pub fn new(
        template_name: impl Into<String>,
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

    async fn prepare(&self, ctx: &PagePrepareContext<'_>) -> Result<Option<PageRenderSpec>>;
}
