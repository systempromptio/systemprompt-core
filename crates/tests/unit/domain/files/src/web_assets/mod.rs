mod pipeline;
mod sharing;
mod standards;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use systemprompt_extension::{AssetDefinition, AssetPaths, Extension};
use systemprompt_files::FilesExtension;

struct NoPaths;

impl AssetPaths for NoPaths {
    fn storage_files(&self) -> &Path {
        Path::new("/nonexistent/storage")
    }

    fn web_dist(&self) -> &Path {
        Path::new("/nonexistent/dist")
    }
}

fn declared() -> Vec<AssetDefinition> {
    FilesExtension.required_assets(&NoPaths)
}

fn assets_root() -> PathBuf {
    let css = declared()
        .into_iter()
        .find(|a| a.source().ends_with("css/documents.css"))
        .expect("documents.css is declared");
    css.source()
        .parent()
        .and_then(Path::parent)
        .expect("assets root")
        .to_path_buf()
}

fn walk(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read assets dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.insert(path);
        }
    }
}

fn shipped_files() -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    walk(&assets_root(), &mut files);
    files.retain(|p| p.file_name().is_some_and(|n| n != "VERSION"));
    files
}

fn own_scripts() -> Vec<(String, String)> {
    let dir = assets_root().join("js");
    shipped_files()
        .into_iter()
        .filter(|p| p.starts_with(&dir))
        .map(|p| {
            let name = p.file_name().expect("name").to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&p).expect("read script");
            (name, text)
        })
        .collect()
}
