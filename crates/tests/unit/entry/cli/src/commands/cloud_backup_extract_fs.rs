//! `cloud backup extract` — the guard that decides what a downloaded tarball
//! is allowed to write.
//!
//! This is a security boundary: the archive comes from the network and is
//! unpacked over the user's services tree. It rejects symlinks, absolute
//! paths, `..` traversal, and anything outside the allowed top-level
//! directories. It was at 0% — the guard shipped with nothing asserting it
//! holds.
//!
//! Each refusal test also asserts that nothing was written, because bailing
//! after unpacking is not a refusal.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use systemprompt_cli::cloud::backup::extract::extract_tarball;

// The `tar` crate refuses to *build* an archive containing `..`, an absolute
// path, or a symlink escaping the tree — so the hostile archives this guard
// exists to reject cannot be produced through its API. These write raw ustar
// headers instead, which is what an attacker would send.
const BLOCK: usize = 512;

fn octal(buf: &mut [u8], value: u64) {
    let last = buf.len() - 1;
    let text = format!("{value:0last$o}");
    buf[..last].copy_from_slice(text.as_bytes());
    buf[last] = 0;
}

fn raw_header(path: &str, size: u64, type_flag: u8, link_target: &str) -> [u8; BLOCK] {
    let mut h = [0u8; BLOCK];
    let name = path.as_bytes();
    assert!(
        name.len() < 100,
        "test paths stay inside the ustar name field"
    );
    h[..name.len()].copy_from_slice(name);
    octal(&mut h[100..108], 0o644);
    octal(&mut h[108..116], 0);
    octal(&mut h[116..124], 0);
    octal(&mut h[124..136], size);
    octal(&mut h[136..148], 0);
    h[156] = type_flag;
    let link = link_target.as_bytes();
    h[157..157 + link.len()].copy_from_slice(link);
    h[257..262].copy_from_slice(b"ustar");
    h[263..265].copy_from_slice(b"00");

    h[148..156].copy_from_slice(&[b' '; 8]);
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    let text = format!("{sum:06o}");
    h[148..154].copy_from_slice(text.as_bytes());
    h[154] = 0;
    h[155] = b' ';
    h
}

fn gzip(raw: Vec<u8>) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::none());
    encoder.write_all(&raw).expect("gzip write");
    encoder.finish().expect("gzip finish")
}

fn tarball(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut raw = Vec::new();
    for (path, body) in entries {
        raw.extend_from_slice(&raw_header(path, body.len() as u64, b'0', ""));
        raw.extend_from_slice(body);
        let rem = body.len() % BLOCK;
        if rem != 0 {
            raw.extend(std::iter::repeat_n(0u8, BLOCK - rem));
        }
    }
    raw.extend(std::iter::repeat_n(0u8, BLOCK * 2));
    gzip(raw)
}

fn symlink_tarball(link_path: &str, link_target: &str) -> Vec<u8> {
    let mut raw = raw_header(link_path, 0, b'2', link_target).to_vec();
    raw.extend(std::iter::repeat_n(0u8, BLOCK * 2));
    gzip(raw)
}

struct Target {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn target() -> Target {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().canonicalize().expect("canonicalize target");
    Target { _dir: dir, path }
}

fn written_files(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !path.is_symlink() {
                walk(&path, root, out);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

// Why: the allow-list is the whole guard. If a well-formed archive did not
// unpack, every refusal test below would pass for the wrong reason.
#[test]
fn an_archive_inside_the_allowed_directories_unpacks() {
    let t = target();
    let data = tarball(&[
        ("agents/one.yaml", b"name: one"),
        ("skills/deep/nested/two.md", b"# two"),
    ]);

    let count = extract_tarball(&data, &t.path).expect("a well-formed archive should unpack");

    assert_eq!(count, 2, "both entries should be reported as extracted");
    assert_eq!(
        written_files(&t.path),
        vec!["agents/one.yaml", "skills/deep/nested/two.md"],
        "the files should land at their archive paths"
    );
}

// Why: `..` is the classic traversal. Unpacking it would write outside the
// services tree entirely — over a dotfile, a binary, anything the user can
// write to.
#[test]
fn a_parent_directory_traversal_is_refused_and_writes_nothing() {
    let t = target();
    let data = tarball(&[("agents/../../escaped.yaml", b"owned")]);

    let err = extract_tarball(&data, &t.path).expect_err("`..` must not unpack");

    assert!(
        format!("{err:#}").contains("invalid path"),
        "the refusal should name the path problem: {err:#}"
    );
    assert!(
        written_files(&t.path).is_empty(),
        "a refused archive must leave nothing behind"
    );
}

#[test]
fn an_absolute_path_is_refused_and_writes_nothing() {
    let t = target();
    let data = tarball(&[("/etc/cron.d/pwn", b"* * * * * root sh")]);

    let err = extract_tarball(&data, &t.path).expect_err("an absolute path must not unpack");

    assert!(
        format!("{err:#}").contains("invalid path"),
        "the refusal should name the path problem: {err:#}"
    );
    assert!(written_files(&t.path).is_empty());
}

// Why: a path can be relative, traversal-free and still not ours. The
// allow-list is what stops the archive writing into a sibling tree it was
// never meant to touch.
#[test]
fn a_top_level_directory_outside_the_allow_list_is_refused() {
    let t = target();
    let data = tarball(&[("secrets/keys.pem", b"-----BEGIN")]);

    let err =
        extract_tarball(&data, &t.path).expect_err("an unlisted top-level dir must not unpack");

    assert!(
        format!("{err:#}").contains("allowed top-level directory"),
        "the refusal should say the directory is not allowed: {err:#}"
    );
    assert!(written_files(&t.path).is_empty());
}

// Why: a file at the archive root has no top-level directory to check, so it
// cannot be inside the allow-list. It must be refused rather than falling
// through the `unwrap_or("")` default.
#[test]
fn a_file_at_the_archive_root_is_refused() {
    let t = target();
    let data = tarball(&[("loose.yaml", b"loose")]);

    let err = extract_tarball(&data, &t.path).expect_err("a rootless file must not unpack");

    assert!(
        format!("{err:#}").contains("allowed top-level directory"),
        "the refusal should say the directory is not allowed: {err:#}"
    );
    assert!(written_files(&t.path).is_empty());
}

// Why: a symlink passes every path check — its own path is ordinary — while
// its target is not checked at all. Unpacked, it turns a later write into a
// write wherever it points. Entry type is checked before path for this reason.
#[test]
fn a_symlink_entry_is_refused_whatever_it_points_at() {
    let t = target();
    let data = symlink_tarball("agents/link.yaml", "/etc/passwd");

    let err = extract_tarball(&data, &t.path).expect_err("a symlink must not unpack");

    assert!(
        format!("{err:#}").contains("disallowed entry type"),
        "the refusal should name the entry type: {err:#}"
    );
    assert!(
        written_files(&t.path).is_empty(),
        "no link should be created"
    );
}

// Why: the guard bails on the first bad entry. An archive that opens with
// valid files and then traverses must not leave the valid ones behind — a
// partial unpack of a hostile archive is still a compromised tree.
#[test]
fn a_hostile_entry_after_valid_ones_still_fails_the_whole_archive() {
    let t = target();
    let data = tarball(&[
        ("agents/good.yaml", b"fine"),
        ("agents/../../escaped.yaml", b"owned"),
    ]);

    let err = extract_tarball(&data, &t.path).expect_err("the archive must be refused");

    assert!(format!("{err:#}").contains("invalid path"));
    assert_eq!(
        written_files(&t.path),
        vec!["agents/good.yaml"],
        "documents current behaviour: entries before the bad one are already on \
         disk when the guard bails, so callers must treat a failed extract as a \
         dirty target rather than a no-op"
    );
}

#[test]
fn an_empty_archive_extracts_nothing_and_is_not_an_error() {
    let t = target();
    let data = tarball(&[]);

    let count = extract_tarball(&data, &t.path).expect("an empty archive is not a failure");

    assert_eq!(count, 0);
    assert!(written_files(&t.path).is_empty());
}
