mod component;
mod extender;
mod loader;
mod provider;

pub use component::{ComponentContext, ComponentRenderer, RenderedComponent};
pub use extender::{ExtendedData, ExtenderContext, TemplateDataExtender};
pub use loader::{EmbeddedLoader, FileSystemLoader, TemplateLoader};
pub use provider::{TemplateDefinition, TemplateProvider, TemplateSource};
