//! A temporary home, configuration directory and repository, and loading of
//! one directory's configuration from them.

use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_engine::config::{AgentDef, AgentMode, Layout, Loaded, Paths, Ruleset};

pub(crate) struct Tree {
    _dir: tempfile::TempDir,
    pub(crate) paths: Paths,
    pub(crate) repo: PathBuf,
}

impl Tree {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let at = |name: &str| root.join(name);
        let paths = Paths {
            home: at("home"),
            data: at("data"),
            config: at("config"),
            cache: at("cache"),
            state: at("state"),
            tmp: at("tmp"),
        };
        let repo = at("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(&paths.home).unwrap();
        Self {
            _dir: dir,
            paths,
            repo,
        }
    }

    pub(crate) fn put(&self, path: &Path, text: &str) -> PathBuf {
        write(path, text);
        path.to_path_buf()
    }

    pub(crate) fn project(&self, rel: &str, text: &str) -> PathBuf {
        self.put(&self.repo.join(rel), text)
    }

    pub(crate) fn global(&self, rel: &str, text: &str) -> PathBuf {
        self.put(&self.paths.config.join(rel), text)
    }

    pub(crate) fn load(&self) -> Loaded {
        self.load_at(&self.repo)
    }

    pub(crate) fn load_at(&self, cwd: &Path) -> Loaded {
        Loaded::load(&self.paths, &Layout::default(), cwd, builtins(&self.paths)).unwrap()
    }

    pub(crate) fn load_with(&self, layout: &Layout) -> Loaded {
        Loaded::load(&self.paths, layout, &self.repo, builtins(&self.paths)).unwrap()
    }
}

pub(crate) fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Two primary agents and one subagent, standing in for the shell's built-ins.
pub(crate) fn builtins(paths: &Paths) -> Vec<AgentDef> {
    let agent = |name: &str, mode: AgentMode| {
        let mut agent = AgentDef::new(name, mode);
        agent.defaults = Ruleset::build_defaults(paths);
        agent
    };
    vec![
        agent("build", AgentMode::Primary),
        agent("plan", AgentMode::Primary),
        agent("general", AgentMode::Subagent),
    ]
}

/// Moves a file's modification time forward so a fingerprint taken in the same
/// clock tick still sees the edit.
pub(crate) fn touch_later(path: &Path) {
    let file = fs::File::options().append(true).open(path).unwrap();
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    file.set_modified(later).unwrap();
}
