//! [`TemplateProvider`] contract for surfacing template definitions to the
//! template registry.

use std::path::PathBuf;

/// Source of a [`TemplateDefinition`] body.
#[derive(Debug, Clone)]
pub enum TemplateSource {
    /// Source embedded in the binary at compile time.
    Embedded(&'static str),
    /// Source loaded from a single file at runtime.
    File(PathBuf),
    /// Source loaded from a directory of files at runtime.
    Directory(PathBuf),
}

/// One template registered with the host's template registry.
#[derive(Debug, Clone)]
pub struct TemplateDefinition {
    /// Template name as referenced from other templates / pages.
    pub name: String,
    /// Source of the template body.
    pub source: TemplateSource,
    /// Registration priority; higher overrides lower.
    pub priority: u32,
    /// Content-type names this template applies to; empty means "all".
    pub content_types: Vec<String>,
}

impl TemplateDefinition {
    /// Build an embedded [`TemplateDefinition`] with default priority.
    #[must_use]
    pub fn embedded(name: impl Into<String>, content: &'static str) -> Self {
        Self {
            name: name.into(),
            source: TemplateSource::Embedded(content),
            priority: 100,
            content_types: vec![],
        }
    }

    /// Build a single-file [`TemplateDefinition`] with default priority.
    #[must_use]
    pub fn file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: TemplateSource::File(path.into()),
            priority: 100,
            content_types: vec![],
        }
    }

    /// Build a directory [`TemplateDefinition`] with default priority.
    #[must_use]
    pub fn directory(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: TemplateSource::Directory(path.into()),
            priority: 100,
            content_types: vec![],
        }
    }

    /// Override the registration priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Replace the full content-type whitelist.
    #[must_use]
    pub fn for_content_types(mut self, types: Vec<String>) -> Self {
        self.content_types = types;
        self
    }

    /// Append a single content-type to the whitelist.
    #[must_use]
    pub fn for_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_types.push(content_type.into());
        self
    }
}

/// Hook that exposes a set of templates to the template registry.
pub trait TemplateProvider: Send + Sync {
    /// Templates exposed by this provider.
    fn templates(&self) -> Vec<TemplateDefinition>;

    /// Stable identifier for this provider.
    fn provider_id(&self) -> &'static str;

    /// Provider priority; higher overrides lower.
    fn priority(&self) -> u32 {
        100
    }
}
