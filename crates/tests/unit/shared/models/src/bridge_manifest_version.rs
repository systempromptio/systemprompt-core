use std::collections::BTreeSet;

use systemprompt_models::bridge::manifest_version::{ManifestVersion, ManifestVersionParseError};

const ONE: &str = "2026-01-01T00:00:00Z-aaaaaaaa";
const TWO: &str = "2026-06-01T00:00:00Z-bbbbbbbb";
const SAME_TS_LOW: &str = "2026-06-01T00:00:00Z-00000000";
const SAME_TS_HIGH: &str = "2026-06-01T00:00:00Z-ffffffff";

#[test]
fn manifest_version_accepts_valid_form() {
    let v = ManifestVersion::try_new(ONE).unwrap();
    assert_eq!(v.as_str(), ONE);
    assert_eq!(v.to_string(), ONE);
}

#[test]
fn manifest_version_rejects_missing_separator() {
    let err = ManifestVersion::try_new("nodash").unwrap_err();
    assert!(matches!(err, ManifestVersionParseError::NoSeparator(_)));
    assert!(err.to_string().contains("missing"));
}

#[test]
fn manifest_version_rejects_bad_timestamp() {
    let err = ManifestVersion::try_new("not-a-date-deadbeef").unwrap_err();
    assert!(matches!(
        err,
        ManifestVersionParseError::BadTimestamp { .. }
    ));
}

#[test]
fn manifest_version_rejects_short_hex_suffix() {
    let err = ManifestVersion::try_new("2026-01-01T00:00:00Z-abc").unwrap_err();
    assert!(matches!(err, ManifestVersionParseError::BadSuffix(_)));
}

#[test]
fn manifest_version_rejects_non_hex_suffix() {
    let err = ManifestVersion::try_new("2026-01-01T00:00:00Z-zzzzzzzz").unwrap_err();
    assert!(matches!(err, ManifestVersionParseError::BadSuffix(_)));
}

#[test]
fn manifest_version_orders_by_timestamp_then_suffix() {
    let v_old = ManifestVersion::try_new(ONE).unwrap();
    let v_new = ManifestVersion::try_new(TWO).unwrap();
    assert!(v_old < v_new);

    let lo = ManifestVersion::try_new(SAME_TS_LOW).unwrap();
    let hi = ManifestVersion::try_new(SAME_TS_HIGH).unwrap();
    assert!(lo < hi);
}

#[test]
fn manifest_version_partial_ord_matches_ord() {
    let a = ManifestVersion::try_new(ONE).unwrap();
    let b = ManifestVersion::try_new(TWO).unwrap();
    assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
}

#[test]
fn manifest_version_equality_and_hash() {
    let a = ManifestVersion::try_new(ONE).unwrap();
    let b = ManifestVersion::try_new(ONE).unwrap();
    assert_eq!(a, b);

    let mut set: BTreeSet<ManifestVersion> = BTreeSet::new();
    set.insert(ManifestVersion::try_new(TWO).unwrap());
    set.insert(ManifestVersion::try_new(ONE).unwrap());
    let first = set.iter().next().unwrap();
    assert_eq!(first.as_str(), ONE);
}

#[test]
fn manifest_version_serde_round_trip_via_try_from() {
    let v = ManifestVersion::try_new(ONE).unwrap();
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, format!("\"{ONE}\""));
    let back: ManifestVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(back.as_str(), ONE);
}

#[test]
fn manifest_version_serde_rejects_invalid_string() {
    let err = serde_json::from_str::<ManifestVersion>("\"bad\"").unwrap_err();
    assert!(err.to_string().contains("missing"));
}

#[test]
fn manifest_version_try_from_string_and_into_string() {
    let v: ManifestVersion = ONE.to_owned().try_into().unwrap();
    let s: String = v.into();
    assert_eq!(s, ONE);
}
