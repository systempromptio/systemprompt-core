// `sanitize_path_segment` is the boundary between manifest-derived strings
// (which can contain colons / slashes / other NTFS-reserved characters) and
// the on-disk path joined into `cowork_plugins/cache/`. Earlier the manifest
// version (`2026-05-28T09:56:34Z-…`) was joined raw and triggered Windows
// ERROR_INVALID_NAME (os error 123), aborting `publish()` mid-way. These
// tests prove every NTFS-reserved character is stripped.

use systemprompt_bridge::integration::cowork_plugins::sanitize_path_segment;

const NTFS_RESERVED: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

#[test]
fn manifest_version_with_colons_is_safe() {
    let raw = "2026-05-28T09:56:34Z-0000019e6e03abad";
    let safe = sanitize_path_segment(raw);
    for c in NTFS_RESERVED {
        assert!(
            !safe.contains(*c),
            "sanitised segment {safe:?} still contains reserved char {c:?}"
        );
    }
    // Should still be recognisable for debugging.
    assert!(safe.contains("2026-05-28T09"));
}

#[test]
fn every_ntfs_reserved_char_is_replaced() {
    let raw: String = NTFS_RESERVED.iter().collect();
    let safe = sanitize_path_segment(&raw);
    for c in NTFS_RESERVED {
        assert!(!safe.contains(*c), "reserved char {c:?} survived");
    }
}

#[test]
fn safe_chars_pass_through() {
    let raw = "abcXYZ123._-";
    assert_eq!(sanitize_path_segment(raw), raw);
}

#[test]
fn unicode_and_whitespace_become_dashes() {
    // Conservative allow-list — anything outside [A-Za-z0-9._-] becomes `-`.
    // This is fine for adapter-internal cache paths; the manifest stays raw.
    let raw = "weird 漢字 v1.0";
    let safe = sanitize_path_segment(raw);
    assert!(safe.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')));
    assert!(safe.contains("v1.0"));
}

#[test]
fn empty_input_yields_empty_output() {
    assert_eq!(sanitize_path_segment(""), "");
}

#[test]
fn determinism_same_input_same_output() {
    let a = sanitize_path_segment("2026-05-28T09:56:34Z");
    let b = sanitize_path_segment("2026-05-28T09:56:34Z");
    assert_eq!(a, b);
}
