//! [`PagePrerenderer`] contract for emitting one statically-rendered page.

use std::any::Any;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ProviderResult;
use crate::web_config::WebConfig;

/// Per-call context handed to a [`PagePrerenderer`].
#[derive(Debug)]
pub struct PagePrepareContext<'a> {
    /// Resolved web config for the rendering host.
    pub web_config: &'a WebConfig,
    content_config: &'a (dyn Any + Send + Sync),
    db_pool: &'a (dyn Any + Send + Sync),
    dist_dir: &'a std::path::Path,
}

impl<'a> PagePrepareContext<'a> {
    /// Build a [`PagePrepareContext`] from its parts.
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

    /// Type-erased downcast of the host's content config.
    #[must_use]
    pub fn content_config<T: 'static>(&self) -> Option<&T> {
        self.content_config.downcast_ref::<T>()
    }

    /// Type-erased downcast of the host's database pool.
    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }

    /// Output root directory for static-site emission.
    #[must_use]
    pub const fn dist_dir(&self) -> &std::path::Path {
        self.dist_dir
    }
}

/// Output description for a single prerendered page.
#[derive(Debug, Clone)]
pub struct PageRenderSpec {
    /// Template name to render against `base_data`.
    pub template_name: String,
    /// Initial template-data payload.
    pub base_data: Value,
    /// Path under the dist root to write the rendered HTML to.
    pub output_path: PathBuf,
}

impl PageRenderSpec {
    /// Build a [`PageRenderSpec`] from its parts.
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

/// Type alias for an `Arc<dyn PagePrerenderer>`.
pub type DynPagePrerenderer = std::sync::Arc<dyn PagePrerenderer>;

/// Hook that prepares one page for static-site emission.
///
/// Marked `#[async_trait]` because it is consumed via `dyn PagePrerenderer`.
#[async_trait]
pub trait PagePrerenderer: Send + Sync {
    /// Logical page type, e.g. `homepage`, `blog`.
    fn page_type(&self) -> &str;

    /// Provider priority; higher runs first.
    fn priority(&self) -> u32 {
        100
    }

    /// Compute the [`PageRenderSpec`], or `None` to skip emission.
    async fn prepare(&self, ctx: &PagePrepareContext<'_>)
    -> ProviderResult<Option<PageRenderSpec>>;
}
