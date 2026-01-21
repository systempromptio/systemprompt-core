mod error;
mod loader;

pub use error::{Result as TemplateLoaderResult, TemplateLoaderError};
pub use loader::{EmbeddedLoader, FileSystemLoader, TemplateLoader};
pub use systemprompt_provider_contracts::{
    ComponentContext, ComponentRenderer, ExtendedData, ExtenderContext, PageContext,
    PageDataProvider, RenderedComponent, TemplateDataExtender, TemplateDefinition,
    TemplateProvider, TemplateSource,
};
