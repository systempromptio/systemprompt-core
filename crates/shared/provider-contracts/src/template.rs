use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum TemplateSource {
    Embedded(&'static str),
    File(PathBuf),
    Directory(PathBuf),
}

#[derive(Debug, Clone)]
pub struct TemplateDefinition {
    pub name: String,
    pub source: TemplateSource,
    pub priority: u32,
    pub content_types: Vec<String>,
}

impl TemplateDefinition {
    #[must_use]
    pub fn embedded(name: impl Into<String>, content: &'static str) -> Self {
        Self {
            name: name.into(),
            source: TemplateSource::Embedded(content),
            priority: 100,
            content_types: vec![],
        }
    }

    #[must_use]
    pub fn file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: TemplateSource::File(path.into()),
            priority: 100,
            content_types: vec![],
        }
    }

    #[must_use]
    pub fn directory(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: TemplateSource::Directory(path.into()),
            priority: 100,
            content_types: vec![],
        }
    }

    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub fn for_content_types(mut self, types: Vec<String>) -> Self {
        self.content_types = types;
        self
    }

    #[must_use]
    pub fn for_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_types.push(content_type.into());
        self
    }
}

pub trait TemplateProvider: Send + Sync {
    fn templates(&self) -> Vec<TemplateDefinition>;

    fn provider_id(&self) -> &'static str;

    fn priority(&self) -> u32 {
        100
    }
}
