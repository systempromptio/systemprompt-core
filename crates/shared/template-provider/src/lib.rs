pub mod traits;

pub use traits::{
    ComponentContext, ComponentRenderer, EmbeddedLoader, ExtendedData, ExtenderContext,
    FileSystemLoader, PageContext, PageDataProvider, RenderedComponent, TemplateDataExtender,
    TemplateDefinition, TemplateLoader, TemplateProvider, TemplateSource,
};

pub type DynTemplateProvider = std::sync::Arc<dyn TemplateProvider>;

pub type DynTemplateLoader = std::sync::Arc<dyn TemplateLoader>;

pub type DynTemplateDataExtender = std::sync::Arc<dyn TemplateDataExtender>;

pub type DynComponentRenderer = std::sync::Arc<dyn ComponentRenderer>;

pub type DynPageDataProvider = std::sync::Arc<dyn PageDataProvider>;
