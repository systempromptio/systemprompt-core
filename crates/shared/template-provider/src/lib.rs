pub mod traits;

pub use traits::{
    ComponentContext, ComponentRenderer, EmbeddedLoader, ExtendedData, ExtenderContext,
    FileSystemLoader, RenderedComponent, TemplateDataExtender, TemplateDefinition, TemplateLoader,
    TemplateProvider, TemplateSource,
};

pub type DynTemplateProvider = std::sync::Arc<dyn TemplateProvider>;

pub type DynTemplateLoader = std::sync::Arc<dyn TemplateLoader>;

pub type DynTemplateDataExtender = std::sync::Arc<dyn TemplateDataExtender>;

pub type DynComponentRenderer = std::sync::Arc<dyn ComponentRenderer>;
