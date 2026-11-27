//! Reads that remember what they looked at, so a load can be fingerprinted
//! afterwards.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::error::ConfigError;

/// A cheap digest of the size and modification time of every file and directory
/// a load consulted, including the ones that did not exist. Equal fingerprints
/// mean nothing the load depends on has changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(u64);

impl Fingerprint {
    pub const fn value(self) -> u64 {
        self.0
    }

    pub(crate) fn of(inputs: &[PathBuf]) -> Self {
        let mut hasher = DefaultHasher::new();
        for path in inputs {
            path.hash(&mut hasher);
            match fs::metadata(path) {
                Ok(meta) => {
                    true.hash(&mut hasher);
                    meta.is_dir().hash(&mut hasher);
                    meta.len().hash(&mut hasher);
                    let modified = meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
                    modified.map(|elapsed| elapsed.as_nanos()).hash(&mut hasher);
                },
                Err(_) => false.hash(&mut hasher),
            }
        }
        Self(hasher.finish())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) is_file: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Probe {
    inputs: BTreeSet<PathBuf>,
    listed: usize,
}

const LIST_BUDGET: usize = 4096;

fn absent(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

impl Probe {
    pub(crate) fn note(&mut self, path: &Path) {
        self.inputs.insert(path.to_path_buf());
    }

    pub(crate) fn read_bytes(&mut self, path: &Path) -> Result<Option<Vec<u8>>, ConfigError> {
        self.note(path);
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if absent(&error) => Ok(None),
            Err(error) if path.is_dir() => {
                let path = path.display();
                tracing::debug!(path = %path, %error, "expected a file, found a directory");
                Ok(None)
            },
            Err(source) => Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    pub(crate) fn read(&mut self, path: &Path) -> Result<Option<String>, ConfigError> {
        Ok(self
            .read_bytes(path)?
            .map(|bytes| match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
            }))
    }

    pub(crate) fn list(&mut self, dir: &Path) -> Vec<Entry> {
        self.note(dir);
        if self.listed >= LIST_BUDGET {
            tracing::debug!(dir = %dir.display(), "directory listing budget spent");
            return Vec::new();
        }
        self.listed += 1;
        let Ok(read) = fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut entries: Vec<Entry> = read
            .flatten()
            .map(|entry| {
                let path = entry.path();
                Entry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    is_dir: path.is_dir(),
                    is_file: path.is_file(),
                    path,
                }
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        entries
    }

    pub(crate) fn markdown_files(&mut self, dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        self.collect_markdown(dir, 0, &mut out);
        out
    }

    fn collect_markdown(&mut self, dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 6 {
            return;
        }
        for entry in self.list(dir) {
            if entry.name.starts_with('.') {
                continue;
            }
            if entry.is_dir {
                self.collect_markdown(&entry.path, depth + 1, out);
            } else if entry.is_file && is_markdown(&entry.path) {
                out.push(entry.path);
            }
        }
    }

    pub(crate) fn into_inputs(self) -> Vec<PathBuf> {
        self.inputs.into_iter().collect()
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<(), ConfigError> {
    let fail = |source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(fail)?;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = path.with_file_name(format!(".{name}.tmp"));
    fs::write(&temp, contents).map_err(fail)?;
    if let Err(source) = fs::rename(&temp, path) {
        if let Err(error) = fs::remove_file(&temp) {
            tracing::debug!(path = %temp.display(), %error, "temporary settings file left behind");
        }
        return Err(fail(source));
    }
    Ok(())
}
