//! Dynamic extension registry that stores extensions as `Arc<dyn
//! Extension>`.
//!
//! The dynamic registry is the lower-level counterpart of
//! [`crate::TypedExtensionRegistry`]: it accepts `Arc<dyn Extension>`
//! values supplied by either inventory discovery or runtime injection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery;
mod queries;
mod validation;

use crate::Extension;
use crate::error::LoaderError;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::warn;

pub use validation::RESERVED_PATHS;

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

    // Why: Topologically order extensions by [`Extension::dependencies`], breaking
    // ties with [`Extension::priority`] (lower runs first).
    //
    // Missing dependencies are warned and ignored — an extension may
    // optionally depend on another that was not loaded in this build. A
    // dependency cycle returns [`LoaderError::DependencyCycle`] with a
    // human-readable chain (`"A -> B -> A"`).
    pub(crate) fn sort_by_priority(&mut self) -> Result<(), LoaderError> {
        let ids: Vec<String> = self
            .sorted_extensions
            .iter()
            .map(|e| e.id().to_owned())
            .collect();
        let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();

        let mut by_id: HashMap<String, Arc<dyn Extension>> = HashMap::new();
        for ext in self.sorted_extensions.drain(..) {
            by_id.insert(ext.id().to_owned(), ext);
        }

        for (owner, ext) in &by_id {
            for dep in ext.dependencies() {
                if !id_set.contains(dep) {
                    warn!(
                        extension = %owner,
                        missing_dependency = %dep,
                        "Extension declares dependency that is not loaded; treating as optional \
                         and ignoring for ordering"
                    );
                }
            }
        }

        let order = topo_sort(&ids, &by_id)?;

        self.sorted_extensions = order
            .into_iter()
            .filter_map(|id| by_id.remove(&id))
            .collect();
        Ok(())
    }

    pub fn register(&mut self, ext: Arc<dyn Extension>) -> Result<(), LoaderError> {
        let id = ext.id().to_owned();
        if self.extensions.contains_key(&id) {
            return Err(LoaderError::DuplicateExtension(id));
        }
        self.extensions.insert(id, Arc::clone(&ext));
        self.sorted_extensions.push(ext);
        self.sort_by_priority()?;
        Ok(())
    }

    pub fn merge(&mut self, extensions: Vec<Arc<dyn Extension>>) -> Result<(), LoaderError> {
        for ext in extensions {
            self.register(ext)?;
        }
        Ok(())
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

fn topo_sort(
    ids: &[String],
    by_id: &HashMap<String, Arc<dyn Extension>>,
) -> Result<Vec<String>, LoaderError> {
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
            deps.sort_by_key(|d| {
                by_id.get(*d).map_or((u32::MAX, String::new()), |e| {
                    (e.priority(), e.id().to_owned())
                })
            });
            for dep in deps {
                visit(dep, by_id, color, path, out)?;
            }
        }

        path.pop();
        color.insert(node.to_owned(), BLACK);
        out.push(node.to_owned());
        Ok(())
    }

    let mut roots: Vec<&String> = ids.iter().collect();
    roots.sort_by_key(|id| {
        by_id.get(*id).map_or((u32::MAX, String::new()), |e| {
            (e.priority(), e.id().to_owned())
        })
    });

    let mut color: HashMap<String, u8> = HashMap::with_capacity(ids.len());
    let mut path: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::with_capacity(ids.len());
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
