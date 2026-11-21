//! The importer's only filesystem writer.
//!
//! Every byte the importer produces goes through [`Sink`], which is the single
//! place that honours `--dry-run`: in dry-run mode each call validates its
//! inputs and computes the same report but touches nothing, so a dry run and a
//! real run cannot disagree about what would be written.
//!
//! A skill folder is copied through [`Sink::copy_skill_tree`], which drops the
//! dev-only files the kit's [`DevFileFilter`] excludes.
//!
//! Every source read names the tree it is read from (the authoring tree, or
//! the plugin directory for a plugin's files) and is confined to it through
//! `systemprompt_models::path_confine`: a file, or any entry of a copied
//! directory, that resolves outside that tree (a symbolic link out of it
//! included) refuses the import rather than copying the link's target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use systemprompt_models::path_confine::{confine_within, entries_within, read_within};

use crate::dev_files::DevFileFilter;
use crate::error::MarketplaceError;

pub(super) fn read_source(root: &Path, path: &Path) -> Result<String, MarketplaceError> {
    read_within(root, path).map_err(|e| MarketplaceError::import(path, "read", e))
}

#[derive(Debug, Clone, Copy)]
struct CopyRoots<'a> {
    tree: &'a Path,
    filter: &'a Path,
}

#[derive(Debug)]
pub(super) struct Sink {
    root: PathBuf,
    dry_run: bool,
    dev_files: DevFileFilter,
    written: RefCell<BTreeSet<PathBuf>>,
}

impl Sink {
    pub(super) fn new(root: &Path, dry_run: bool, dev_files: DevFileFilter) -> Self {
        Self {
            root: root.to_path_buf(),
            dry_run,
            dev_files,
            written: RefCell::new(BTreeSet::new()),
        }
    }

    pub(super) fn write_bytes(&self, rel: &Path, bytes: &[u8]) -> Result<(), MarketplaceError> {
        self.written.borrow_mut().insert(rel.to_path_buf());
        if self.dry_run {
            return Ok(());
        }
        let dest = self.root.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
        }
        std::fs::write(&dest, bytes).map_err(|e| io_err(&dest, e))
    }

    pub(super) fn write_yaml<T: Serialize>(
        &self,
        rel: &Path,
        value: &T,
    ) -> Result<(), MarketplaceError> {
        let text = serde_yaml::to_string(value)
            .map_err(|e| MarketplaceError::import(rel, "serialise yaml", e))?;
        self.write_bytes(rel, text.as_bytes())
    }

    pub(super) fn copy_file(
        &self,
        root: &Path,
        src: &Path,
        rel: &Path,
    ) -> Result<(), MarketplaceError> {
        let src = &confine_within(root, src)
            .map_err(|e| MarketplaceError::import(src, "confine to the source tree", e))?;
        if !src.is_file() {
            return Err(MarketplaceError::Import {
                path: src.display().to_string(),
                message: "referenced file does not exist".to_owned(),
            });
        }
        self.written.borrow_mut().insert(rel.to_path_buf());
        if self.dry_run {
            return Ok(());
        }
        let dest = self.root.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
        }
        std::fs::copy(src, &dest)
            .map(|_| ())
            .map_err(|e| io_err(&dest, e))
    }

    pub(super) fn copy_tree(
        &self,
        root: &Path,
        src: &Path,
        rel: &Path,
    ) -> Result<(), MarketplaceError> {
        self.copy_filtered(CopyRoots { tree: root, filter: src }, src, rel, None)
    }

    pub(super) fn copy_skill_tree(
        &self,
        root: &Path,
        src: &Path,
        rel: &Path,
    ) -> Result<(), MarketplaceError> {
        self.copy_filtered(
            CopyRoots {
                tree: root,
                filter: src,
            },
            src,
            rel,
            Some(&self.dev_files),
        )
    }

    pub(super) fn copy_plugin_subtree(
        &self,
        plugin_dir: &Path,
        src: &Path,
        rel: &Path,
    ) -> Result<(), MarketplaceError> {
        self.copy_filtered(
            CopyRoots {
                tree: plugin_dir,
                filter: plugin_dir,
            },
            src,
            rel,
            Some(&self.dev_files),
        )
    }

    fn copy_filtered(
        &self,
        roots: CopyRoots<'_>,
        src: &Path,
        rel: &Path,
        filter: Option<&DevFileFilter>,
    ) -> Result<(), MarketplaceError> {
        if !src.is_dir() {
            return Err(MarketplaceError::Import {
                path: src.display().to_string(),
                message: "expected a directory".to_owned(),
            });
        }
        let entries = entries_within(roots.tree, src)
            .map_err(|e| MarketplaceError::import(src, "confine to the source tree", e))?;
        for entry in entries {
            let path = src.join(&entry.name);
            let child = rel.join(&entry.name);
            if filter.is_some_and(|f| f.excludes_on_disk(roots.filter, &path, entry.is_dir)) {
                continue;
            }
            if entry.is_dir {
                self.copy_filtered(roots, &path, &child, filter)?;
            } else {
                self.copy_file(roots.tree, &path, &child)?;
            }
        }
        Ok(())
    }

    pub(super) fn exists(&self, rel: &Path) -> bool {
        self.written.borrow().contains(rel) || self.root.join(rel).exists()
    }
}

fn io_err(path: &Path, e: std::io::Error) -> MarketplaceError {
    MarketplaceError::import(path, "write", e)
}
