use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use harness_workspace::{VcsError, VcsInfo};

use crate::vcs_tests::{repo, stage};

const SIGNATURE: &[u8; 4] = b"DIRC";
const REGULAR_FILE: u32 = 0o100_644;

pub(crate) fn entry_bytes(path: &str, mtime: u32, size: u32, mode: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&mtime.to_be_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&mode.to_be_bytes());
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(&[0xab; 20]);
    let name_len = u16::try_from(path.len()).unwrap();
    out.extend_from_slice(&name_len.to_be_bytes());
    out.extend_from_slice(path.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(8) {
        out.push(0);
    }
    out
}

pub(crate) fn index_bytes(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(SIGNATURE);
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&u32::try_from(entries.len()).unwrap().to_be_bytes());
    for entry in entries {
        out.extend_from_slice(entry);
    }
    out.extend_from_slice(&[0; 20]);
    out
}

fn stat(root: &Path, rel: &str) -> (u32, u32) {
    let metadata = fs::metadata(root.join(rel)).unwrap();
    let mtime = metadata
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    (
        u32::try_from(mtime).unwrap(),
        u32::try_from(metadata.len()).unwrap(),
    )
}

fn version_four_index(root: &Path, entries: &[(u8, &str, &str)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(SIGNATURE);
    bytes.extend_from_slice(&4u32.to_be_bytes());
    bytes.extend_from_slice(&u32::try_from(entries.len()).unwrap().to_be_bytes());
    for (strip, suffix, full) in entries {
        let (mtime, size) = stat(root, full);
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&mtime.to_be_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&REGULAR_FILE.to_be_bytes());
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&size.to_be_bytes());
        bytes.extend_from_slice(&[0xcd; 20]);
        bytes.extend_from_slice(&0u16.to_be_bytes());
        bytes.push(*strip);
        bytes.extend_from_slice(suffix.as_bytes());
        bytes.push(0);
    }
    bytes
}

fn write_index(root: &Path, bytes: &[u8]) -> VcsInfo {
    fs::write(root.join(".git").join("index"), bytes).unwrap();
    VcsInfo::discover(root).unwrap().unwrap()
}

#[test]
fn version_two_entries_with_nested_paths_are_clean() {
    let dir = repo();
    let root = dir.path();
    fs::write(root.join("a.txt"), "abc").unwrap();
    fs::create_dir_all(root.join("dir")).unwrap();
    fs::write(root.join("dir").join("longer-name.rs"), "fn main() {}\n").unwrap();
    let index = index_bytes(&[stage(root, "a.txt"), stage(root, "dir/longer-name.rs")]);
    let info = write_index(root, &index);
    assert!(!info.is_dirty().unwrap());
    fs::write(root.join("dir").join("longer-name.rs"), "fn main() { }\n").unwrap();
    assert!(info.is_dirty().unwrap());
}

#[test]
fn version_four_prefix_compression_names_every_file() {
    let dir = repo();
    let root = dir.path();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("lib.rs"), "lib").unwrap();
    fs::write(root.join("src").join("main.rs"), "main!").unwrap();
    let index = version_four_index(
        root,
        &[
            (0, "src/lib.rs", "src/lib.rs"),
            (6, "main.rs", "src/main.rs"),
        ],
    );
    let info = write_index(root, &index);
    assert!(!info.is_dirty().unwrap());
}

#[test]
fn garbage_index_is_corrupt() {
    let dir = repo();
    let info = write_index(dir.path(), b"nope");
    assert!(matches!(info.is_dirty(), Err(VcsError::CorruptIndex)));
}

#[test]
fn truncated_index_is_rejected() {
    let dir = repo();
    fs::write(dir.path().join("a"), "a").unwrap();
    let full = index_bytes(&[entry_bytes("a", 1, 1, REGULAR_FILE)]);
    let info = write_index(dir.path(), &full[..20]);
    assert!(info.is_dirty().is_err());
}

#[test]
fn unsupported_index_version_is_reported() {
    let dir = repo();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(SIGNATURE);
    bytes.extend_from_slice(&9u32.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    let info = write_index(dir.path(), &bytes);
    assert!(matches!(
        info.is_dirty(),
        Err(VcsError::UnsupportedIndex(9))
    ));
}
