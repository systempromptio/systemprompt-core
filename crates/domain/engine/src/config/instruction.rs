//! Instruction files: `AGENTS.md` (or `CLAUDE.md`) from the global directory
//! and every project level, plus files and globs named in settings, each capped
//! in size.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};

use crate::config::error::ConfigError;
use crate::config::fsio::Probe;
use crate::config::paths::{Layout, Paths, expand_home};

const PRIMARY: &str = "AGENTS.md";
const FALLBACK: &str = "CLAUDE.md";
const GLOB_FILES: usize = 64;
const GLOB_DEPTH: usize = 8;
const SKIPPED_DIRS: [&str; 2] = ["node_modules", "target"];

/// One instruction file as it will be shown to the model. `bytes` is its size
/// on disk; `truncated` is set when `text` holds only the first
/// `Layout::instruction_limit` bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub path: PathBuf,
    pub text: String,
    pub bytes: usize,
    pub truncated: bool,
}

pub(crate) struct Inputs<'a> {
    pub(crate) paths: &'a Paths,
    pub(crate) layout: &'a Layout,
    pub(crate) root: &'a Path,
    pub(crate) chain: &'a [PathBuf],
    pub(crate) extra: &'a [String],
}

pub(crate) fn collect(
    probe: &mut Probe,
    inputs: &Inputs<'_>,
    problems: &mut Vec<ConfigError>,
) -> Vec<Instruction> {
    let mut collector = Collector {
        limit: inputs.layout.instruction_limit,
        seen: HashSet::new(),
        out: Vec::new(),
    };
    let compat = inputs.layout.compat_dirs;
    collector.level(probe, &inputs.paths.config, compat, problems);
    let home_fallback = inputs.paths.home.join(".claude").join(FALLBACK);
    if compat && let Err(problem) = collector.add(probe, &home_fallback) {
        problems.push(problem);
    }
    for dir in inputs.chain {
        collector.level(probe, dir, compat, problems);
    }
    for entry in inputs.extra {
        collector.extra(probe, entry, inputs, problems);
    }
    collector.out
}

struct Collector {
    limit: usize,
    seen: HashSet<PathBuf>,
    out: Vec<Instruction>,
}

impl Collector {
    fn level(
        &mut self,
        probe: &mut Probe,
        dir: &Path,
        compat: bool,
        problems: &mut Vec<ConfigError>,
    ) {
        let names: &[&str] = if compat {
            &[PRIMARY, FALLBACK]
        } else {
            &[PRIMARY]
        };
        for name in names {
            match self.add(probe, &dir.join(name)) {
                Ok(true) => break,
                Ok(false) => {},
                Err(problem) => problems.push(problem),
            }
        }
    }

    fn add(&mut self, probe: &mut Probe, path: &Path) -> Result<bool, ConfigError> {
        let Some(bytes) = probe.read_bytes(path)? else {
            return Ok(false);
        };
        if self.seen.insert(path.to_path_buf()) {
            let size = bytes.len();
            let (text, truncated) = cap(&bytes, self.limit);
            if truncated {
                let limit = self.limit;
                tracing::debug!(path = %path.display(), size, limit, "instruction file truncated");
            }
            self.out.push(Instruction {
                path: path.to_path_buf(),
                text,
                bytes: size,
                truncated,
            });
        }
        Ok(true)
    }

    fn extra(
        &mut self,
        probe: &mut Probe,
        entry: &str,
        inputs: &Inputs<'_>,
        problems: &mut Vec<ConfigError>,
    ) {
        let expanded = expand_home(entry.trim(), &inputs.paths.home);
        let pattern = expanded.to_string_lossy().replace('\\', "/");
        let (base, rest) = split_glob(&pattern);
        let base = if Path::new(&base).is_absolute() {
            PathBuf::from(&base)
        } else {
            inputs.root.join(&base)
        };
        if rest.is_empty() {
            match self.add(probe, &base) {
                Ok(true) => {},
                Ok(false) => problems.push(ConfigError::InstructionMissing { path: base }),
                Err(problem) => problems.push(problem),
            }
            return;
        }
        let built = GlobBuilder::new(&rest)
            .literal_separator(true)
            .backslash_escape(false)
            .build();
        let matcher = match built {
            Ok(glob) => glob.compile_matcher(),
            Err(source) => {
                problems.push(ConfigError::Glob {
                    path: base,
                    pattern: entry.to_owned(),
                    source,
                });
                return;
            },
        };
        let mut search = GlobSearch {
            base: &base,
            matcher: &matcher,
            found: Vec::new(),
        };
        search.walk(probe, &base, 0);
        let mut found = search.found;
        found.sort();
        found.truncate(GLOB_FILES);
        for file in found {
            if let Err(problem) = self.add(probe, &file) {
                problems.push(problem);
            }
        }
    }
}

struct GlobSearch<'a> {
    base: &'a Path,
    matcher: &'a GlobMatcher,
    found: Vec<PathBuf>,
}

impl GlobSearch<'_> {
    fn walk(&mut self, probe: &mut Probe, dir: &Path, depth: usize) {
        if depth > GLOB_DEPTH || self.found.len() >= GLOB_FILES * 4 {
            return;
        }
        for entry in probe.list(dir) {
            if entry.name.starts_with('.') || SKIPPED_DIRS.contains(&entry.name.as_str()) {
                continue;
            }
            if entry.is_dir {
                self.walk(probe, &entry.path, depth + 1);
            } else if entry.is_file
                && let Ok(relative) = entry.path.strip_prefix(self.base)
                && self.matcher.is_match(relative)
            {
                self.found.push(entry.path);
            }
        }
    }
}

fn split_glob(pattern: &str) -> (String, String) {
    let parts: Vec<&str> = pattern.split('/').collect();
    let first_glob = parts
        .iter()
        .position(|part| part.contains(['*', '?', '[', '{']))
        .unwrap_or(parts.len());
    let base = parts[..first_glob].join("/");
    let base = if base.is_empty() && pattern.starts_with('/') {
        "/".to_owned()
    } else {
        base
    };
    (base, parts[first_glob..].join("/"))
}

fn cap(bytes: &[u8], limit: usize) -> (String, bool) {
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    if text.len() <= limit {
        return (text, false);
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    (text, true)
}
