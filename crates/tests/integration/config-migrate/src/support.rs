//! Scratch services trees built from fixtures or inline legacy YAML.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub struct Tree {
    _dir: TempDir,
    pub services: PathBuf,
}

impl Tree {
    pub fn root(&self) -> PathBuf {
        self.services.join("config").join("config.yaml")
    }
}

pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../common/fixtures/catalog")
}

fn root_config(ai_config: bool) -> &'static str {
    if ai_config {
        "includes:\n  - ../ai/config.yaml\n"
    } else {
        "{}\n"
    }
}

pub fn from_files(files: &[(&str, &str)]) -> Tree {
    let dir = TempDir::new().expect("tempdir");
    let services = dir.path().join("services");
    std::fs::create_dir_all(services.join("ai")).expect("ai dir");
    std::fs::create_dir_all(services.join("config")).expect("config dir");
    for (name, body) in files {
        std::fs::write(services.join("ai").join(name), body).expect("write fixture file");
    }
    let has_ai = files.iter().any(|(name, _)| *name == "config.yaml");
    std::fs::write(services.join("config/config.yaml"), root_config(has_ai)).expect("root");
    Tree {
        _dir: dir,
        services,
    }
}

pub fn from_fixture(fixture: &str) -> Tree {
    let legacy = fixtures_root().join(fixture).join("legacy/ai");
    let mut files: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&legacy).expect("fixture dir") {
        let path = entry.expect("entry").path();
        let name = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        files.push((name, std::fs::read_to_string(&path).expect("read")));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    from_files(&borrowed)
}

pub fn snapshot(services: &Path) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = walk(services)
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            (p, text)
        })
        .collect();
    out.sort();
    out
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}
