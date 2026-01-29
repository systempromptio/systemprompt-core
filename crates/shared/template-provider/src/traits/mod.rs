mod error;
mod loader;

pub use error::{Result as TemplateLoaderResult, TemplateLoaderError};
#[cfg(feature = "tokio")]
pub use loader::FileSystemLoader;
pub use loader::{EmbeddedLoader, TemplateLoader};
pub use systemprompt_provider_contracts::{
    ComponentContext, ComponentRenderer, ExtendedData, ExtenderContext, PageContext,
    PageDataProvider, PartialSource, PartialTemplate, RenderedComponent, TemplateDataExtender,
    TemplateDefinition, TemplateProvider, TemplateSource,
};
