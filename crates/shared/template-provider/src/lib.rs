pub mod traits;

pub use traits::{
    ComponentContext, ComponentRenderer, EmbeddedLoader, ExtendedData, ExtenderContext,
    PageContext, PageDataProvider, RenderedComponent, TemplateDataExtender, TemplateDefinition,
    TemplateLoader, TemplateLoaderError, TemplateLoaderResult, TemplateProvider, TemplateSource,
};

pub use systemprompt_provider_contracts::{PagePrepareContext, PagePrerenderer, PageRenderSpec};

#[cfg(feature = "tokio")]
pub use traits::FileSystemLoader;

pub type DynTemplateProvider = std::sync::Arc<dyn TemplateProvider>;

pub type DynTemplateLoader = std::sync::Arc<dyn TemplateLoader>;

pub type DynTemplateDataExtender = std::sync::Arc<dyn TemplateDataExtender>;

pub type DynComponentRenderer = std::sync::Arc<dyn ComponentRenderer>;

pub type DynPageDataProvider = std::sync::Arc<dyn PageDataProvider>;

pub type DynPagePrerenderer = std::sync::Arc<dyn PagePrerenderer>;
