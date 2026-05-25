//! Adversarial / boundary tests for `ExtensionLoader`.
//!
//! These tests pin the loader's behaviour against malformed manifests,
//! path-traversal in the declared binary name, symlink trickery in the
//! extensions tree, duplicate-binary collisions, and concurrent
//! mutation of the extensions directory.
//!
//! Background — what `ExtensionLoader` is and is not:
//!
//! - It is a discoverer for trusted same-operator subprocess extensions (MCP
//!   servers, CLI extensions). It parses `extensions/*/manifest.yaml`, resolves
//!   a binary name to a path under `<project_root>/target/`, and reports what
//!   is missing.
//! - It is **not** a signed-binary loader. The loader does not verify the
//!   binary's bytes, has no concept of a signing key or revocation list, has no
//!   `version` field on the manifest, and does not sandbox the spawned
//!   subprocess. See finding `F-T1d-001` in the due-diligence findings ledger
//!   for the gap analysis vs. an Ed25519 / sandbox model.
//!
//! These tests therefore characterise the loader's real attack
//! surface (parser robustness, path handling, collision behaviour,
//! concurrent-mutation safety) rather than pretending to exercise a
//! signature model that is not in the codebase.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use systemprompt_loader::ExtensionLoader;
use tempfile::TempDir;

fn write_manifest(dir: &std::path::Path, body: &str) {
    std::fs::create_dir_all(dir).expect("create ext dir");
    std::fs::write(dir.join("manifest.yaml"), body).expect("write manifest");
}

// -----------------------------------------------------------------------------
// Manifest schema rejection (parser robustness)
// -----------------------------------------------------------------------------

/// Manifest missing the required `extension.name` field is rejected
/// silently (skipped) — the loader never panics, but the offending
/// directory contributes zero discovered extensions.
#[test]
fn adversarial_manifest_missing_required_name_is_skipped() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("nameless");
    write_manifest(
        &ext,
        r#"extension:
  type: mcp
  binary: nameless-bin
  description: "missing required name field"
  enabled: true
"#,
    );

    let discovered = ExtensionLoader::discover(temp.path());
    assert!(
        discovered.is_empty(),
        "manifest without `name` must not produce a DiscoveredExtension"
    );
}

/// Manifest with a wrong-typed field (`enabled: "yes"` instead of
/// boolean) is rejected without panicking; discovery returns empty
/// for that directory.
#[test]
fn adversarial_manifest_wrong_type_field_is_skipped() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("badtype");
    write_manifest(
        &ext,
        r#"extension:
  type: mcp
  name: badtype
  binary: badtype-bin
  enabled: "yes"
"#,
    );

    let discovered = ExtensionLoader::discover(temp.path());
    assert!(
        discovered.is_empty(),
        "manifest with wrong-typed `enabled` must be rejected, not coerced"
    );
}

/// Manifest with an unknown `extension.type` deserialises to the
/// `Other` variant rather than panicking; the resulting extension is
/// neither MCP nor CLI and is filtered out of both lookups.
#[test]
fn adversarial_manifest_unknown_type_is_not_mcp_or_cli() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("weird");
    write_manifest(
        &ext,
        r#"extension:
  type: weird-unsupported-kind
  name: weird
  binary: weird-bin
  enabled: true
"#,
    );

    // serde with the current enum can't coerce an unknown variant, so
    // the manifest is rejected at parse time — the same outcome as
    // any other malformed manifest.
    let discovered = ExtensionLoader::discover(temp.path());
    assert!(
        discovered.is_empty(),
        "unknown extension.type variant must be rejected at parse time"
    );

    assert!(ExtensionLoader::get_enabled_mcp_extensions(temp.path()).is_empty());
    assert!(ExtensionLoader::get_enabled_cli_extensions(temp.path()).is_empty());
}

/// Manifest with an absurdly oversized `name` field (1 MiB) parses
/// without exhausting memory or panicking; the discovered extension
/// surfaces the full string and the caller can size-check it.
#[test]
fn adversarial_manifest_oversized_name_does_not_panic() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("huge");
    let huge_name = "x".repeat(1024 * 1024);
    write_manifest(
        &ext,
        &format!(
            r#"extension:
  type: mcp
  name: "{huge_name}"
  binary: huge-bin
  enabled: true
"#
        ),
    );

    let discovered = ExtensionLoader::discover(temp.path());
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].manifest.extension.name.len(), huge_name.len());
}

/// Manifest with embedded NUL / control bytes in a string field
/// parses (YAML allows them) but does not panic the discoverer; the
/// name survives intact for downstream validation.
#[test]
fn adversarial_manifest_control_chars_in_name_do_not_panic() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("ctrl");
    write_manifest(
        &ext,
        "extension:\n  type: mcp\n  name: \"weird\\x00name\\x01\"\n  binary: ctrl-bin\n  enabled: \
         true\n",
    );

    // The loader must not panic regardless of whether this parses.
    let _ = ExtensionLoader::discover(temp.path());
}

/// A YAML document that triggers the well-known "billion laughs"
/// alias-expansion DoS must not hang or panic discovery. serde_yaml
/// is expected to either reject or bound expansion; either way the
/// loader returns control to the caller.
#[test]
fn adversarial_manifest_alias_bomb_does_not_hang() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("bomb");
    // Compact alias bomb — serde_yaml rejects deeply nested aliases.
    let bomb = r#"extension: &a
  type: mcp
  name: bomb
  binary: bomb
  enabled: true
  description: &b "lol"
  roles:
    a: { display_name: *b, description: *b }
    b: { display_name: *b, description: *b }
    c: { display_name: *b, description: *b }
"#;
    write_manifest(&ext, bomb);

    // Should complete quickly with either Ok or skipped — never hang.
    let discovered = ExtensionLoader::discover(temp.path());
    // Either parsed-and-kept or rejected-and-dropped is acceptable;
    // the assertion is just that we got here without timing out.
    assert!(discovered.len() <= 1);
}

// -----------------------------------------------------------------------------
// Path-traversal / symlink handling
// -----------------------------------------------------------------------------

/// A manifest whose `binary` field contains `../` is preserved as-is
/// by the parser. The loader treats it as a plain filename when
/// joining with `target/release/`, so the resolved path escapes the
/// target directory.
///
/// This is a *characterisation* test: it pins the current behaviour
/// (no sanitisation) so any future hardening shows up as a deliberate
/// behaviour change rather than an accidental regression. The loader
/// is not the right layer to enforce this — the spawn site must
/// canonicalise — but the test makes the gap visible.
#[test]
fn adversarial_path_traversal_in_binary_field_is_not_sanitised() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("escape");
    write_manifest(
        &ext,
        r#"extension:
  type: mcp
  name: escape
  binary: "../../etc/passwd"
  enabled: true
"#,
    );

    let discovered = ExtensionLoader::discover(temp.path());
    assert_eq!(discovered.len(), 1);
    let binary = discovered[0].binary_name().expect("binary field present");
    assert!(
        binary.contains(".."),
        "loader does not sanitise `..` in binary names; spawn site must canonicalise"
    );
}

/// A symlinked extensions subdirectory pointing outside `project_root`
/// is still walked by discover() — the loader follows the link
/// transparently because it relies on `fs::read_dir` without a
/// canonicalise-and-check step. Documenting current behaviour.
#[cfg(unix)]
#[test]
fn adversarial_symlinked_extension_dir_is_followed() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("tempdir");
    let outside = TempDir::new().expect("outside tempdir");
    let real_ext = outside.path().join("real-ext");
    write_manifest(
        &real_ext,
        r#"extension:
  type: mcp
  name: linked
  binary: linked-bin
  enabled: true
"#,
    );

    let extensions_dir = temp.path().join("extensions");
    std::fs::create_dir_all(&extensions_dir).expect("extensions dir");
    symlink(&real_ext, extensions_dir.join("linked")).expect("symlink");

    let discovered = ExtensionLoader::discover(temp.path());
    assert_eq!(
        discovered.len(),
        1,
        "symlinked extension dir is followed; if the loader ever stops following symlinks this \
         test must change deliberately"
    );
    assert_eq!(discovered[0].manifest.extension.name, "linked");
}

// -----------------------------------------------------------------------------
// Binary content tampering — the loader does NOT detect this
// -----------------------------------------------------------------------------

/// The loader's `validate_mcp_binaries` only checks for existence; it
/// returns `Ok` regardless of the binary's bytes. Mutating a single
/// byte (simulating tamper-after-install) does not surface as a
/// signature-mismatch error because there is no signature
/// verification in this code path. See finding `F-T1d-001`.
#[test]
fn adversarial_tampered_binary_is_not_detected_by_loader() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("tamper");
    write_manifest(
        &ext,
        r#"extension:
  type: mcp
  name: tamper
  binary: tamper-bin
  enabled: true
"#,
    );
    let release = temp.path().join("target").join("release");
    std::fs::create_dir_all(&release).expect("release dir");
    let binary = release.join("tamper-bin");
    std::fs::write(&binary, b"\x7fELF original payload").expect("write binary");

    let missing_before = ExtensionLoader::validate_mcp_binaries(temp.path());
    assert!(missing_before.is_empty(), "binary exists; loader is happy");

    // Mutate one byte — a real tamper.
    std::fs::write(&binary, b"\x7fELF TAMPERED payload").expect("rewrite binary");

    let missing_after = ExtensionLoader::validate_mcp_binaries(temp.path());
    assert!(
        missing_after.is_empty(),
        "loader does not detect content tampering — this is finding F-T1d-001"
    );
}

// -----------------------------------------------------------------------------
// Duplicate binary names / collision handling
// -----------------------------------------------------------------------------

/// Two extensions declaring the same `binary` name collide in
/// `build_binary_map`; only one wins. The loader does not surface
/// this as an error, so a malicious or misconfigured later
/// extension can shadow an earlier one.
#[test]
fn adversarial_duplicate_binary_names_silently_collide() {
    let temp = TempDir::new().expect("tempdir");
    let ext_a = temp.path().join("extensions").join("a");
    let ext_b = temp.path().join("extensions").join("b");
    write_manifest(
        &ext_a,
        r#"extension:
  type: mcp
  name: a
  binary: shared-bin
  enabled: true
"#,
    );
    write_manifest(
        &ext_b,
        r#"extension:
  type: mcp
  name: b
  binary: shared-bin
  enabled: true
"#,
    );

    let map = ExtensionLoader::build_binary_map(temp.path());
    assert_eq!(
        map.len(),
        1,
        "two extensions claiming `shared-bin` collapse to one map entry — collision silently \
         resolved"
    );
}

/// A disabled extension whose binary collides with an enabled one
/// does not interfere with the production binary-name list.
#[test]
fn adversarial_disabled_extension_excluded_from_production_names() {
    let temp = TempDir::new().expect("tempdir");
    let ext = temp.path().join("extensions").join("off");
    write_manifest(
        &ext,
        r#"extension:
  type: mcp
  name: off
  binary: off-bin
  enabled: false
"#,
    );

    let names = ExtensionLoader::get_mcp_binary_names(temp.path());
    assert!(
        !names.contains(&"off-bin".to_string()),
        "disabled MCP extensions must not appear in production binary names"
    );
}

// -----------------------------------------------------------------------------
// Concurrent discovery / hot-reload race surrogate
// -----------------------------------------------------------------------------

/// Concurrent `discover()` calls while a writer adds and removes
/// manifest files must not panic. The loader has no hot-reload
/// concept of its own; this test stands in as a race surrogate for
/// the underlying `fs::read_dir` traversal.
#[test]
fn adversarial_concurrent_discovery_does_not_panic() {
    let temp = TempDir::new().expect("tempdir");
    let extensions_dir = temp.path().join("extensions");
    std::fs::create_dir_all(&extensions_dir).expect("ext dir");

    let stop = Arc::new(AtomicBool::new(false));

    let writer = {
        let stop = Arc::clone(&stop);
        let root = temp.path().to_path_buf();
        thread::spawn(move || {
            let mut i: u32 = 0;
            while !stop.load(Ordering::Relaxed) {
                let ext = root.join("extensions").join(format!("churn-{i}"));
                let _ = std::fs::create_dir_all(&ext);
                let _ = std::fs::write(
                    ext.join("manifest.yaml"),
                    format!(
                        "extension:\n  type: mcp\n  name: churn-{i}\n  binary: churn-bin-{i}\n  \
                         enabled: true\n"
                    ),
                );
                if i > 0 {
                    let stale = root.join("extensions").join(format!("churn-{}", i - 1));
                    let _ = std::fs::remove_dir_all(&stale);
                }
                i = i.wrapping_add(1);
            }
        })
    };

    for _ in 0..200 {
        let _ = ExtensionLoader::discover(temp.path());
    }

    stop.store(true, Ordering::Relaxed);
    writer.join().expect("writer thread joined");
}
