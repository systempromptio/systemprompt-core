//! Extension registry that stores extensions as `Arc<dyn Extension>`.
//!
//! It accepts `Arc<dyn Extension>` values supplied by either inventory
//! discovery or runtime injection and orders them once: a depth-first walk of
//! `dependencies()` that visits roots and siblings by `(priority, id)`, so a
//! dependency always precedes its dependents and ties break deterministically.
//! That one order drives schema installation, router and job collection and
//! [`ExtensionRegistry::build_state`]. A declared dependency that is not
//! loaded is a [`LoaderError::MissingDependency`] from every constructor, so
//! no caller can observe a registry whose dependency graph is unsatisfied.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery;
mod provide;
mod queries;
mod validation;

use crate::Extension;
use crate::error::LoaderError;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::ExtensionId;

pub use validation::{RESERVED_PATHS, WEB_ROOT_BASE_PATH};

#[derive(Default)]
pub struct ExtensionRegistry {
    pub(crate) extensions: HashMap<String, Arc<dyn Extension>>,
    pub(crate) sorted_extensions: Vec<Arc<dyn Extension>>,
}

impl std::fmt::Debug for ExtensionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionRegistry")
            .field("extension_count", &self.extensions.len())
            .finish_non_exhaustive()
    }
}

impl ExtensionRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn sort_by_priority(&mut self) -> Result<(), LoaderError> {
        self.sorted_extensions = dependency_order(&self.extensions)?
            .into_iter()
            .filter_map(|id| self.extensions.get(&id).cloned())
            .collect();
        Ok(())
    }

    pub fn register(&mut self, ext: Arc<dyn Extension>) -> Result<(), LoaderError> {
        self.insert(ext)?;
        self.sort_by_priority()?;
        Ok(())
    }

    pub(crate) fn insert(&mut self, ext: Arc<dyn Extension>) -> Result<(), LoaderError> {
        let id = ext.id().to_owned();
        if self.extensions.contains_key(&id) {
            return Err(LoaderError::DuplicateExtension(ExtensionId::new(id)));
        }
        self.extensions.insert(id, Arc::clone(&ext));
        self.sorted_extensions.push(ext);
        Ok(())
    }

    pub fn merge(&mut self, extensions: Vec<Arc<dyn Extension>>) -> Result<(), LoaderError> {
        for ext in extensions {
            self.insert(ext)?;
        }
        self.sort_by_priority()
    }

    pub fn validate(&self) -> Result<(), LoaderError> {
        self.validate_dependencies()?;
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.extensions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}

pub(crate) fn dependency_order(
    by_id: &HashMap<String, Arc<dyn Extension>>,
) -> Result<Vec<String>, LoaderError> {
    let mut owners: Vec<(&String, &Arc<dyn Extension>)> = by_id.iter().collect();
    owners.sort_by(|a, b| a.0.cmp(b.0));
    for (owner, ext) in owners {
        for dep in ext.dependencies() {
            if !by_id.contains_key(dep) {
                return Err(LoaderError::MissingDependency {
                    extension: ExtensionId::new(owner.as_str()),
                    dependency: ExtensionId::new(dep),
                });
            }
        }
    }
    topo_sort(by_id)
}

fn order_key(by_id: &HashMap<String, Arc<dyn Extension>>, id: &str) -> (u32, String) {
    by_id.get(id).map_or((u32::MAX, String::new()), |e| {
        (e.priority(), e.id().to_owned())
    })
}

fn topo_sort(by_id: &HashMap<String, Arc<dyn Extension>>) -> Result<Vec<String>, LoaderError> {
    const WHITE: u8 = 0;
    const GRAY: u8 = 1;
    const BLACK: u8 = 2;

    fn visit(
        node: &str,
        by_id: &HashMap<String, Arc<dyn Extension>>,
        color: &mut HashMap<String, u8>,
        path: &mut Vec<String>,
        out: &mut Vec<String>,
    ) -> Result<(), LoaderError> {
        let state = color.get(node).copied().unwrap_or(WHITE);
        if state == BLACK {
            return Ok(());
        }
        if state == GRAY {
            let cycle_start = path.iter().position(|p| p == node).unwrap_or(0);
            let mut chain: Vec<String> = path[cycle_start..].to_vec();
            chain.push(node.to_owned());
            return Err(LoaderError::DependencyCycle {
                chain: chain.join(" -> "),
            });
        }
        color.insert(node.to_owned(), GRAY);
        path.push(node.to_owned());

        if let Some(ext) = by_id.get(node) {
            let mut deps: Vec<&'static str> = ext
                .dependencies()
                .into_iter()
                .filter(|d| by_id.contains_key(*d))
                .collect();
            deps.sort_by_key(|d| order_key(by_id, d));
            for dep in deps {
                visit(dep, by_id, color, path, out)?;
            }
        }

        path.pop();
        color.insert(node.to_owned(), BLACK);
        out.push(node.to_owned());
        Ok(())
    }

    let mut roots: Vec<&String> = by_id.keys().collect();
    roots.sort_by_key(|id| order_key(by_id, id));

    let mut color: HashMap<String, u8> = HashMap::with_capacity(by_id.len());
    let mut path: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::with_capacity(by_id.len());
    for id in roots {
        visit(id, by_id, &mut color, &mut path, &mut out)?;
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
pub struct ExtensionRegistration {
    pub factory: fn() -> Arc<dyn Extension>,
}

inventory::collect!(ExtensionRegistration);
