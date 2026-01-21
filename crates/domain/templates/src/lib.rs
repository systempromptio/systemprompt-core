mod builder;
mod core_provider;
mod error;
mod registry;

pub use builder::TemplateRegistryBuilder;
pub use core_provider::CoreTemplateProvider;
pub use error::TemplateError;
pub use registry::{RegistryStats, TemplateRegistry};

pub use systemprompt_template_provider::{
    ComponentContext, ComponentRenderer, DynComponentRenderer, DynPageDataProvider,
    DynTemplateDataExtender, DynTemplateLoader, DynTemplateProvider, EmbeddedLoader, ExtendedData,
    ExtenderContext, FileSystemLoader, PageContext, PageDataProvider, RenderedComponent,
    TemplateDataExtender, TemplateDefinition, TemplateLoader, TemplateProvider, TemplateSource,
};
