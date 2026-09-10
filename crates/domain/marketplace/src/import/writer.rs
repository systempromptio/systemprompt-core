//! The importer's only filesystem writer.
//!
//! Every byte the importer produces goes through [`Sink`], which is the single
//! place that honours `--dry-run`: in dry-run mode each call validates its
//! inputs and computes the same report but touches nothing, so a dry run and a
//! real run cannot disagree about what would be written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::MarketplaceError;

#[derive(Debug)]
pub(super) struct Sink {
    root: PathBuf,
    dry_run: bool,
    written: RefCell<BTreeSet<PathBuf>>,
}

impl Sink {
    pub(super) fn new(root: &Path, dry_run: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            dry_run,
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
            std::fs::create_dir_all(parent).map_err(|e| io_err(parent, &e))?;
        }
        std::fs::write(&dest, bytes).map_err(|e| io_err(&dest, &e))
    }

    pub(super) fn write_yaml<T: Serialize>(
        &self,
        rel: &Path,
        value: &T,
    ) -> Result<(), MarketplaceError> {
        let text = serde_yaml::to_string(value).map_err(|e| MarketplaceError::Import {
            path: rel.display().to_string(),
            message: e.to_string(),
        })?;
        self.write_bytes(rel, text.as_bytes())
    }

    pub(super) fn copy_file(&self, src: &Path, rel: &Path) -> Result<(), MarketplaceError> {
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
            std::fs::create_dir_all(parent).map_err(|e| io_err(parent, &e))?;
        }
        std::fs::copy(src, &dest)
            .map(|_| ())
            .map_err(|e| io_err(&dest, &e))
    }

    pub(super) fn copy_tree(&self, src: &Path, rel: &Path) -> Result<(), MarketplaceError> {
        if !src.is_dir() {
            return Err(MarketplaceError::Import {
                path: src.display().to_string(),
                message: "expected a directory".to_owned(),
            });
        }
        for entry in std::fs::read_dir(src).map_err(|e| io_err(src, &e))? {
            let entry = entry.map_err(|e| io_err(src, &e))?;
            let path = entry.path();
            let Some(name) = path.file_name() else {
                continue;
            };
            let child = rel.join(name);
            if path.is_dir() {
                self.copy_tree(&path, &child)?;
            } else {
                self.copy_file(&path, &child)?;
            }
        }
        Ok(())
    }

    pub(super) fn exists(&self, rel: &Path) -> bool {
        self.written.borrow().contains(rel) || self.root.join(rel).exists()
    }
}

fn io_err(path: &Path, e: &std::io::Error) -> MarketplaceError {
    MarketplaceError::Import {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}
