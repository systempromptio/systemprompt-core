//! Digests of the corpus's normalisation rules: a text digest over the text
//! with line endings, trailing blanks and trailing newlines normalised, and a
//! tree digest over relative paths and file contents.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub fn text(value: &str) -> String {
    let unified = value.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = unified
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect();
    let joined = lines.join("\n");
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(joined.trim_end_matches('\n').as_bytes()))
    )
}

fn content(bytes: &[u8]) -> Vec<u8> {
    match std::str::from_utf8(bytes) {
        Ok(text) if !text.contains('\0') => {
            text.replace("\r\n", "\n").replace('\r', "\n").into_bytes()
        },
        _ => bytes.to_vec(),
    }
}

/// Every file under `root`, by relative forward-slash path, with its
/// normalised content; directories named in `skip` (relative) are left out.
pub fn files(root: &Path, skip: &[&str]) -> anyhow::Result<BTreeMap<String, Vec<u8>>> {
    let mut found = BTreeMap::new();
    if !root.exists() {
        return Ok(found);
    }
    for entry in WalkDir::new(root).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(root)?;
        let path: Vec<String> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        if path.first().is_some_and(|top| skip.contains(&top.as_str())) {
            continue;
        }
        found.insert(path.join("/"), content(&std::fs::read(entry.path())?));
    }
    Ok(found)
}

pub fn tree(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut listing = Vec::new();
    for (path, bytes) in files {
        listing.extend_from_slice(path.as_bytes());
        listing.push(0);
        listing.extend_from_slice(hex::encode(Sha256::digest(bytes)).as_bytes());
        listing.push(b'\n');
    }
    format!("sha256:{}", hex::encode(Sha256::digest(&listing)))
}

pub fn changed(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut paths: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect();
    paths.sort();
    paths.dedup();
    paths
}
