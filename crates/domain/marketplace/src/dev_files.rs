//! Dev-only files a kit keeps out of the client tree.
//!
//! A skill folder often carries what its author needs and its user does not:
//! a `README.md`, a `tests/` tree, fixtures, `*.test.*` specs.
//! [`DevFileFilter`] is the one place that decides which of those files never
//! leave the authoring tree. It is applied where skill files are copied on
//! import, captured into a managed revision, and laid out for a consumer, so a
//! dev file dropped at one point cannot reappear at another.
//!
//! Two rule sets apply:
//!
//! - the defaults, matched against the path inside the skill: `README.md` (any
//!   case) at the skill root; any `tests/`, `test/`, `fixtures/` or
//!   `__tests__/` directory; any `*.test.*` or `*.spec.*` file;
//! - the kit's own [`IGNORE_FILE`] at the kit root, in gitignore syntax (`#`
//!   comments, `!` negation, a trailing `/` for directories, a pattern
//!   containing `/` anchored to the kit root, `**` across directories), matched
//!   against the path relative to the kit root.
//!
//! Each path is judged one directory level at a time, as git does: the last
//! ignore-file pattern matching that level decides it, a level no pattern
//! matches falls back to the defaults, and an excluded directory excludes
//! everything under it. A `!` pattern can therefore re-include a default
//! exclude (`!fixtures/` ships a skill's fixtures) but not a file whose parent
//! directory is excluded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};

pub const IGNORE_FILE: &str = ".systempromptignore";

const DEV_DIRS: &[&str] = &["tests", "test", "fixtures", "__tests__"];
const DEV_INFIXES: &[&str] = &["test", "spec"];

#[derive(Debug, thiserror::Error)]
pub enum DevFileError {
    #[error("{path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("{path}:{line}: invalid ignore pattern: {message}")]
    Pattern {
        path: String,
        line: usize,
        message: String,
    },
}

#[derive(Debug, Clone)]
struct IgnoreRule {
    matcher: GlobMatcher,
    negate: bool,
    dir_only: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DevFileFilter {
    kit_root: Option<PathBuf>,
    rules: Vec<IgnoreRule>,
}

impl DevFileFilter {
    pub const fn defaults() -> Self {
        Self {
            kit_root: None,
            rules: Vec::new(),
        }
    }

    pub fn load(kit_root: &Path) -> Result<Self, DevFileError> {
        let path = kit_root.join(IGNORE_FILE);
        let rules = if path.is_file() {
            let text = std::fs::read_to_string(&path).map_err(|source| DevFileError::Io {
                path: path.display().to_string(),
                source,
            })?;
            parse_rules(&text).map_err(|(line, message)| DevFileError::Pattern {
                path: path.display().to_string(),
                line,
                message,
            })?
        } else {
            Vec::new()
        };
        Ok(Self {
            kit_root: Some(kit_root.to_path_buf()),
            rules,
        })
    }

    pub fn excludes(&self, skill_rel: &str, kit_rel: Option<&str>, is_dir: bool) -> bool {
        let parts: Vec<&str> = skill_rel.split('/').filter(|p| !p.is_empty()).collect();
        let prefix = kit_rel
            .and_then(|kit| kit.strip_suffix(skill_rel))
            .unwrap_or_default();
        (1..=parts.len()).any(|depth| {
            let candidate = parts[..depth].join("/");
            let candidate_is_dir = depth < parts.len() || is_dir;
            self.rule_verdict(&format!("{prefix}{candidate}"), candidate_is_dir)
                .unwrap_or_else(|| {
                    excluded_by_default(parts[depth - 1], candidate_is_dir, depth == 1)
                })
        })
    }

    pub fn excludes_on_disk(&self, skill_root: &Path, path: &Path, is_dir: bool) -> bool {
        let Some(skill_rel) = relative(skill_root, path) else {
            return false;
        };
        let kit_rel = self
            .kit_root
            .as_deref()
            .and_then(|root| relative(root, path));
        self.excludes(&skill_rel, kit_rel.as_deref(), is_dir)
    }

    fn rule_verdict(&self, candidate: &str, is_dir: bool) -> Option<bool> {
        self.rules
            .iter()
            .rev()
            .find(|rule| (is_dir || !rule.dir_only) && rule.matcher.is_match(candidate))
            .map(|rule| !rule.negate)
    }
}

fn excluded_by_default(name: &str, is_dir: bool, at_skill_root: bool) -> bool {
    if is_dir {
        return DEV_DIRS.contains(&name);
    }
    if at_skill_root && name.eq_ignore_ascii_case("README.md") {
        return true;
    }
    let segments: Vec<&str> = name.split('.').collect();
    segments.len() > 2
        && segments[1..segments.len() - 1]
            .iter()
            .any(|segment| DEV_INFIXES.contains(segment))
}

fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn parse_rules(text: &str) -> Result<Vec<IgnoreRule>, (usize, String)> {
    let mut rules = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (negate, line) = line
            .strip_prefix('!')
            .map_or((false, line), |rest| (true, rest));
        let line = line.strip_prefix('\\').unwrap_or(line);
        let (dir_only, line) = line
            .strip_suffix('/')
            .map_or((false, line), |rest| (true, rest));
        if line.is_empty() {
            continue;
        }
        let glob = if line.contains('/') {
            line.trim_start_matches('/').to_owned()
        } else {
            format!("**/{line}")
        };
        let matcher = GlobBuilder::new(&glob)
            .literal_separator(true)
            .backslash_escape(true)
            .build()
            .map_err(|e| (index + 1, e.to_string()))?
            .compile_matcher();
        rules.push(IgnoreRule {
            matcher,
            negate,
            dir_only,
        });
    }
    Ok(rules)
}
