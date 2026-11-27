use std::fs;
use std::path::{Path, PathBuf};

use harness_config::{AgentDef, Layout, Loaded, Paths};

pub(crate) fn paths(root: &Path) -> Paths {
    let dir = |name: &str| root.join(name);
    Paths {
        home: dir("home"),
        data: dir("data"),
        config: dir("config"),
        cache: dir("cache"),
        state: dir("state"),
        tmp: dir("tmp"),
    }
}

pub(crate) fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

pub(crate) fn repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    repo
}

pub(crate) fn load(paths: &Paths, cwd: &Path) -> Loaded {
    Loaded::load(paths, &Layout::default(), cwd, Vec::new()).unwrap()
}

pub(crate) fn load_with(paths: &Paths, cwd: &Path, builtins: Vec<AgentDef>) -> Loaded {
    Loaded::load(paths, &Layout::default(), cwd, builtins).unwrap()
}
