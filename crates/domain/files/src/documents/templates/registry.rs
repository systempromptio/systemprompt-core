//! The merged template registry and template instantiation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_identifiers::DocumentTemplateId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, DocumentTemplate, DocumentTemplateProvider, TemplateParams,
    TemplateSource, registered_template_providers,
};

use super::catalog::{CatalogTemplate, TemplateCatalogError};
use super::core::{CORE_TEMPLATE_PRIORITY, GeneratedBlank, office_blanks};
use crate::documents::codecs::CodecRegistry;

#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("document template {id} does not exist")]
    NotFound { id: DocumentTemplateId },
    #[error("no codec is registered for {format} documents")]
    NoCodec { format: DocumentFormat },
    #[error("template catalog is invalid")]
    Catalog(#[from] TemplateCatalogError),
    #[error("template could not be instantiated")]
    Codec(#[from] CodecError),
    #[error("failed to read template file {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("template placeholder `{key}` is required")]
    MissingPlaceholder { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantiatedTemplate {
    pub template_id: DocumentTemplateId,
    pub format: DocumentFormat,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct Entry {
    template: DocumentTemplate,
    priority: u32,
    generate: Option<fn() -> Vec<u8>>,
}

/// Every known document template, one per id.
///
/// Built once at a composition root from the inventory providers and the
/// services catalog; the highest priority wins per id and, on equal
/// priority, the source merged first. `instantiate` reads `ServicesPath`
/// files synchronously with `std::fs` at document-create time; async callers
/// run it on a blocking pool.
#[derive(Debug, Clone, Default)]
pub struct TemplateRegistry {
    by_id: BTreeMap<DocumentTemplateId, Entry>,
}

impl TemplateRegistry {
    #[must_use]
    pub fn discover(catalog: Vec<CatalogTemplate>) -> Self {
        let mut registry = Self::from_sources(registered_template_providers(), catalog);
        for blank in office_blanks() {
            registry = registry.with_generated(blank, CORE_TEMPLATE_PRIORITY);
        }
        registry
    }

    #[must_use]
    pub fn from_sources(
        providers: impl IntoIterator<Item = Arc<dyn DocumentTemplateProvider>>,
        catalog: Vec<CatalogTemplate>,
    ) -> Self {
        let mut registry = Self::default();
        for provider in providers {
            let priority = provider.priority();
            for template in provider.templates() {
                registry.insert(Entry {
                    template,
                    priority,
                    generate: None,
                });
            }
        }
        for entry in catalog {
            registry.insert(Entry {
                template: entry.template,
                priority: entry.priority,
                generate: None,
            });
        }
        registry
    }

    #[must_use]
    pub fn with_generated(mut self, blank: GeneratedBlank, priority: u32) -> Self {
        self.insert(Entry {
            template: blank.template,
            priority,
            generate: Some(blank.generate),
        });
        self
    }

    fn insert(&mut self, entry: Entry) {
        match self.by_id.get(&entry.template.id) {
            Some(existing) if existing.priority >= entry.priority => {},
            _ => {
                self.by_id.insert(entry.template.id.clone(), entry);
            },
        }
    }

    #[must_use]
    pub fn get(&self, id: &DocumentTemplateId) -> Option<&DocumentTemplate> {
        self.by_id.get(id).map(|entry| &entry.template)
    }

    #[must_use]
    pub fn list(&self) -> Vec<&DocumentTemplate> {
        let mut templates: Vec<&DocumentTemplate> =
            self.by_id.values().map(|entry| &entry.template).collect();
        templates.sort_by(|a, b| {
            a.format
                .as_str()
                .cmp(b.format.as_str())
                .then_with(|| a.name.cmp(&b.name))
        });
        templates
    }

    #[must_use]
    pub fn list_for(&self, format: DocumentFormat) -> Vec<&DocumentTemplate> {
        self.list()
            .into_iter()
            .filter(|template| template.format == format)
            .collect()
    }

    pub fn load_bytes(&self, id: &DocumentTemplateId) -> Result<Vec<u8>, TemplateError> {
        let entry = self
            .by_id
            .get(id)
            .ok_or_else(|| TemplateError::NotFound { id: id.clone() })?;
        if let Some(generate) = entry.generate {
            return Ok(generate());
        }
        match &entry.template.source {
            TemplateSource::Embedded(bytes) => Ok(bytes.to_vec()),
            TemplateSource::ServicesPath(path) => {
                std::fs::read(path).map_err(|source| TemplateError::Io {
                    path: path.clone(),
                    source,
                })
            },
        }
    }

    pub fn instantiate(
        &self,
        id: &DocumentTemplateId,
        params: &TemplateParams,
        codecs: &CodecRegistry,
    ) -> Result<InstantiatedTemplate, TemplateError> {
        let template = self
            .get(id)
            .ok_or_else(|| TemplateError::NotFound { id: id.clone() })?;
        let mut resolved = params.clone();
        for placeholder in &template.placeholders {
            if resolved.values.contains_key(&placeholder.key) {
                continue;
            }
            match &placeholder.default {
                Some(default) => {
                    resolved
                        .values
                        .insert(placeholder.key.clone(), default.clone());
                },
                None => {
                    return Err(TemplateError::MissingPlaceholder {
                        key: placeholder.key.clone(),
                    });
                },
            }
        }
        let codec = codecs
            .for_format(template.format)
            .ok_or(TemplateError::NoCodec {
                format: template.format,
            })?;
        let base = self.load_bytes(id)?;
        let bytes = codec.instantiate(&base, &resolved)?;
        Ok(InstantiatedTemplate {
            template_id: template.id.clone(),
            format: template.format,
            bytes,
        })
    }
}
