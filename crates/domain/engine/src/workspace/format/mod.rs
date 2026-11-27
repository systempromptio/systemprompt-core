//! Formatting a file after it was written: pick the first configured or
//! built-in formatter whose extension matches, whose root marker (if any) is
//! found above the file and whose program resolves, run it with a timeout, and
//! report whether the file's bytes changed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod builtin;
mod error;
mod locate;
mod process;
mod spec;

use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use parking_lot::Mutex;

pub use builtin::builtin_formatters;
pub use error::FormatError;
use locate::{expand_args, find_marker, local_bin};
pub use spec::{FormatterConfig, FormatterSpec, ResolvedFormatter};

/// The formatters of one worktree.
#[derive(Debug)]
pub struct Formatter {
    worktree: PathBuf,
    enabled: bool,
    specs: Vec<FormatterSpec>,
    timeout: Duration,
    search_path: Option<OsString>,
    path_cache: Mutex<HashMap<String, Option<PathBuf>>>,
}

impl Formatter {
    pub fn new(worktree: impl Into<PathBuf>, config: FormatterConfig) -> Self {
        let is_disabled = |name: &str| config.disabled.iter().any(|d| d == name);
        let mut specs: Vec<FormatterSpec> = config
            .custom
            .iter()
            .filter(|spec| !is_disabled(&spec.name))
            .cloned()
            .collect();
        if config.builtins {
            for spec in builtin_formatters() {
                let replaced = config.custom.iter().any(|custom| custom.name == spec.name);
                if !replaced && !is_disabled(&spec.name) {
                    specs.push(spec);
                }
            }
        }
        Self {
            worktree: worktree.into(),
            enabled: config.enabled,
            specs,
            timeout: config.timeout,
            search_path: config.search_path,
            path_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn specs(&self) -> &[FormatterSpec] {
        &self.specs
    }

    pub fn candidates(&self, file: &Path) -> Vec<&FormatterSpec> {
        let Some(name) = file.file_name().and_then(|n| n.to_str()) else {
            return Vec::new();
        };
        self.specs
            .iter()
            .filter(|spec| spec.handles(name))
            .collect()
    }

    pub fn resolve(&self, file: &Path) -> Option<ResolvedFormatter> {
        if !self.enabled {
            return None;
        }
        let file = self.absolute(file);
        let start = file.parent()?;
        let boundary = file
            .starts_with(&self.worktree)
            .then_some(self.worktree.as_path());
        for spec in self.candidates(&file) {
            let cwd = if spec.root_markers.is_empty() {
                start.to_path_buf()
            } else {
                match find_marker(start, &spec.root_markers, boundary) {
                    Some(dir) => dir,
                    None => continue,
                }
            };
            let Some(program) = self.program(spec, start, boundary) else {
                continue;
            };
            return Some(ResolvedFormatter {
                name: spec.name.clone(),
                program,
                args: expand_args(&spec.args, &file),
                cwd,
            });
        }
        None
    }

    pub async fn format(&self, file: &Path) -> Result<bool, FormatError> {
        let file = self.absolute(file);
        let Some(resolved) = self.resolve(&file) else {
            return Ok(false);
        };
        let read_error = |source: io::Error| FormatError::Read {
            path: file.clone(),
            source,
        };
        let before = tokio::fs::read(&file).await.map_err(read_error)?;
        process::run(&resolved, self.timeout).await?;
        let after = tokio::fs::read(&file).await.map_err(read_error)?;
        let changed = before != after;
        tracing::debug!(
            formatter = %resolved.name,
            path = %file.display(),
            changed,
            "formatted file"
        );
        Ok(changed)
    }

    pub fn clear_cache(&self) {
        self.path_cache.lock().clear();
    }

    fn absolute(&self, file: &Path) -> PathBuf {
        if file.is_absolute() {
            file.to_path_buf()
        } else {
            self.worktree.join(file)
        }
    }

    fn program(
        &self,
        spec: &FormatterSpec,
        start: &Path,
        boundary: Option<&Path>,
    ) -> Option<PathBuf> {
        if spec.node_modules
            && let Some(local) = local_bin(start, &spec.command, boundary)
        {
            return Some(local);
        }
        let command = Path::new(&spec.command);
        if command.is_absolute() {
            return command.is_file().then(|| command.to_path_buf());
        }
        if command.components().count() > 1 {
            let candidate = self.worktree.join(command);
            return candidate.is_file().then_some(candidate);
        }
        let mut cache = self.path_cache.lock();
        if let Some(hit) = cache.get(&spec.command) {
            return hit.clone();
        }
        let found = self.search_path.as_ref().map_or_else(
            || which::which(&spec.command).ok(),
            |search_path| which::which_in(&spec.command, Some(search_path), &self.worktree).ok(),
        );
        cache.insert(spec.command.clone(), found.clone());
        found
    }
}
