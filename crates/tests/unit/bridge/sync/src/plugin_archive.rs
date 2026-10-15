//! Plugin archive extraction: only the manifest's files, verified, land in
//! staging.

use std::io::Write;

use sha2::{Digest, Sha256};
use systemprompt_bridge::gateway::manifest::{PluginEntry, PluginFile};
use systemprompt_bridge::host_sync::ApplyError;
use systemprompt_bridge::ids::{PluginId, Sha256Digest};
use systemprompt_bridge::sync::apply::archive::extract_verified;

fn digest(bytes: &[u8]) -> Sha256Digest {
    Sha256Digest::try_new(systemprompt_bridge::hash::hex_encode(&Sha256::digest(
        bytes,
    )))
    .unwrap()
}

fn plugin(files: &[(&str, &[u8])]) -> PluginEntry {
    PluginEntry {
        id: PluginId::try_new("acme-plugin").unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        files: files
            .iter()
            .map(|(path, bytes)| PluginFile {
                path: (*path).to_owned(),
                sha256: digest(bytes),
                size: bytes.len() as u64,
            })
            .collect(),
        hooks: systemprompt_models::plugin::PluginHooksRef::default(),
    }
}

enum Entry<'a> {
    File(&'a str, &'a [u8]),
    Symlink(&'a str, &'a str),
}

// Why: tar::Builder refuses `..` and absolute names, so hostile entries are
// written through the raw header name field the way an attacker would.
fn archive(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for entry in entries {
        let mut header = tar::Header::new_old();
        let (name, data, kind, link): (&str, &[u8], _, Option<&str>) = match entry {
            Entry::File(name, data) => (name, data, tar::EntryType::Regular, None),
            Entry::Symlink(name, target) => (name, b"", tar::EntryType::Symlink, Some(target)),
        };
        let field = &mut header.as_old_mut().name;
        field.fill(0);
        field[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(kind);
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        if let Some(target) = link {
            header.set_link_name(target).unwrap();
        }
        header.set_cksum();
        builder.append(&header, data).unwrap();
    }
    let tar = builder.into_inner().unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&tar).unwrap();
    gz.finish().unwrap()
}

#[test]
fn a_complete_archive_is_staged_byte_for_byte() {
    let files: &[(&str, &[u8])] = &[
        (".claude-plugin/plugin.json", b"{}"),
        ("skills/a/SKILL.md", b"body"),
    ];
    let bytes = archive(&[
        Entry::File("./.claude-plugin/plugin.json", b"{}"),
        Entry::File("skills/a/SKILL.md", b"body"),
    ]);
    let stage = tempfile::tempdir().unwrap();
    extract_verified(&bytes, &plugin(files), stage.path()).unwrap();
    assert_eq!(
        std::fs::read(stage.path().join("skills/a/SKILL.md")).unwrap(),
        b"body"
    );
    assert_eq!(
        std::fs::read(stage.path().join(".claude-plugin/plugin.json")).unwrap(),
        b"{}"
    );
}

#[test]
fn a_traversing_entry_is_refused_and_nothing_escapes() {
    let outer = tempfile::tempdir().unwrap();
    let stage = outer.path().join("stage");
    std::fs::create_dir_all(&stage).unwrap();
    let bytes = archive(&[Entry::File("../escape.txt", b"x")]);
    let err = extract_verified(&bytes, &plugin(&[("escape.txt", b"x")]), &stage).unwrap_err();
    assert!(matches!(err, ApplyError::UnsafePath(_)), "{err}");
    assert!(!outer.path().join("escape.txt").exists());
}

#[test]
fn an_absolute_entry_is_refused() {
    let stage = tempfile::tempdir().unwrap();
    let bytes = archive(&[Entry::File("/etc/passwd", b"x")]);
    let err = extract_verified(&bytes, &plugin(&[("etc/passwd", b"x")]), stage.path()).unwrap_err();
    assert!(matches!(err, ApplyError::UnsafePath(_)), "{err}");
}

#[test]
fn a_symlink_entry_is_refused_even_under_a_manifest_name() {
    let stage = tempfile::tempdir().unwrap();
    let bytes = archive(&[Entry::Symlink("link", "/etc/passwd")]);
    let err = extract_verified(&bytes, &plugin(&[("link", b"")]), stage.path()).unwrap_err();
    assert!(matches!(err, ApplyError::UnsafePath(_)), "{err}");
    assert!(!stage.path().join("link").exists());
}

#[test]
fn an_entry_the_manifest_does_not_name_is_refused() {
    let stage = tempfile::tempdir().unwrap();
    let bytes = archive(&[Entry::File("a.txt", b"a"), Entry::File("extra.txt", b"e")]);
    let err = extract_verified(&bytes, &plugin(&[("a.txt", b"a")]), stage.path()).unwrap_err();
    assert!(matches!(err, ApplyError::UnsafePath(_)), "{err}");
}

#[test]
fn a_body_that_does_not_match_its_digest_fails() {
    let stage = tempfile::tempdir().unwrap();
    let bytes = archive(&[Entry::File("a.txt", b"tampered")]);
    let err =
        extract_verified(&bytes, &plugin(&[("a.txt", b"original")]), stage.path()).unwrap_err();
    assert!(matches!(err, ApplyError::HashMismatch { .. }), "{err}");
}

#[test]
fn a_manifest_file_missing_from_the_archive_fails() {
    let stage = tempfile::tempdir().unwrap();
    let bytes = archive(&[Entry::File("a.txt", b"a")]);
    let err = extract_verified(
        &bytes,
        &plugin(&[("a.txt", b"a"), ("b.txt", b"b")]),
        stage.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("b.txt"), "{err}");
}
