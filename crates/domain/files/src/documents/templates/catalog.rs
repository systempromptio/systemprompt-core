//! The `services/documents/templates/*.yaml` template catalog.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use systemprompt_identifiers::DocumentTemplateId;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_provider_contracts::documents::{
    DocumentFormat, DocumentTemplate, TemplatePlaceholder, TemplateSource,
};

pub const CATALOG_PRIORITY: u32 = 150;

#[derive(Debug, thiserror::Error)]
pub enum TemplateCatalogError {
    #[error("failed to read template catalog path {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid template catalog entry {path}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("template catalog entry {path} has an invalid id")]
    InvalidId {
        path: PathBuf,
        #[source]
        source: IdValidationError,
    },
    #[error("template catalog entry {path} names unknown format `{value}`")]
    UnknownFormat { path: PathBuf, value: String },
    #[error("template file {path} does not exist")]
    MissingFile { path: PathBuf },
    #[error("template id {id} is declared by both {first} and {second}")]
    DuplicateId {
        id: DocumentTemplateId,
        first: PathBuf,
        second: PathBuf,
    },
}

/// A catalog template together with the priority it competes at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogTemplate {
    pub template: DocumentTemplate,
    pub priority: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogEntry {
    id: String,
    format: String,
    name: String,
    #[serde(default)]
    description: String,
    file: PathBuf,
    #[serde(default)]
    priority: Option<u32>,
    #[serde(default)]
    placeholders: Vec<TemplatePlaceholder>,
}

#[must_use]
pub fn catalog_dir(services_root: &Path) -> PathBuf {
    services_root.join("documents").join("templates")
}

pub fn load_catalog(services_root: &Path) -> Result<Vec<CatalogTemplate>, TemplateCatalogError> {
    let dir = catalog_dir(services_root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(&dir).map_err(|source| TemplateCatalogError::Io {
        path: dir.clone(),
        source,
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| TemplateCatalogError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        let is_yaml = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "yaml" || ext == "yml");
        if is_yaml && path.is_file() {
            paths.push(path);
        }
    }
    paths.sort();

    let mut seen: BTreeMap<DocumentTemplateId, PathBuf> = BTreeMap::new();
    let mut templates = Vec::with_capacity(paths.len());
    for path in paths {
        let loaded = load_entry(&path)?;
        if let Some(first) = seen.get(&loaded.template.id) {
            return Err(TemplateCatalogError::DuplicateId {
                id: loaded.template.id,
                first: first.clone(),
                second: path,
            });
        }
        seen.insert(loaded.template.id.clone(), path);
        templates.push(loaded);
    }
    Ok(templates)
}

fn load_entry(path: &Path) -> Result<CatalogTemplate, TemplateCatalogError> {
    let text = std::fs::read_to_string(path).map_err(|source| TemplateCatalogError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let entry: CatalogEntry =
        serde_yaml::from_str(&text).map_err(|source| TemplateCatalogError::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
    let id = DocumentTemplateId::try_new(entry.id).map_err(|source| {
        TemplateCatalogError::InvalidId {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let format: DocumentFormat =
        entry
            .format
            .parse()
            .map_err(|_| TemplateCatalogError::UnknownFormat {
                path: path.to_path_buf(),
                value: entry.format.clone(),
            })?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let file = base.join(&entry.file);
    if !file.is_file() {
        return Err(TemplateCatalogError::MissingFile { path: file });
    }
    let file = std::fs::canonicalize(&file).map_err(|source| TemplateCatalogError::Io {
        path: file.clone(),
        source,
    })?;
    Ok(CatalogTemplate {
        template: DocumentTemplate {
            id,
            format,
            name: entry.name,
            description: entry.description,
            source: TemplateSource::ServicesPath(file),
            placeholders: entry.placeholders,
        },
        priority: entry.priority.unwrap_or(CATALOG_PRIORITY),
    })
}
