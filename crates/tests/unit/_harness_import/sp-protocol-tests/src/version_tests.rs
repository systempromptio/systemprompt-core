use chrono::{DateTime, Duration, Utc};
use sp_protocol::ManifestVersion;
use sp_protocol::version::{dated_beyond_skew, is_newer};

fn version(raw: &str) -> ManifestVersion {
    ManifestVersion::try_new(raw).unwrap()
}

#[test]
fn parses_the_wire_form_and_round_trips() {
    let raw = "2026-11-01T09:12:31Z-000001a19661c117";
    let parsed = version(raw);
    assert_eq!(parsed.as_str(), raw);
    let json = serde_json::to_string(&parsed).unwrap();
    assert_eq!(json, format!("\"{raw}\""));
    let back: ManifestVersion = serde_json::from_str(&json).unwrap();
    assert_eq!(back, parsed);
}

#[test]
fn refuses_malformed_versions() {
    assert!(ManifestVersion::try_new("nodash").is_err());
    assert!(ManifestVersion::try_new("yesterday-0123456789").is_err());
    assert!(ManifestVersion::try_new("2026-11-01T09:12:31Z-abc").is_err());
    assert!(ManifestVersion::try_new("2026-11-01T09:12:31Z-zzzzzzzzzz").is_err());
}

#[test]
fn orders_by_timestamp_then_suffix() {
    let early = version("2026-11-01T09:12:31Z-ffffffffffffffff");
    let late = version("2026-11-01T09:12:32Z-0000000000000000");
    assert!(early < late);
    let low = version("2026-11-01T09:12:31Z-000001a19661c117");
    let high = version("2026-11-01T09:12:31Z-000001a19661c118");
    assert!(low < high);
    let offset = version("2026-11-01T10:12:31+01:00-000001a19661c117");
    assert_eq!(offset.cmp(&low), std::cmp::Ordering::Equal);
}

#[test]
fn only_a_strictly_newer_version_is_accepted() {
    let accepted = version("2026-11-01T09:12:31Z-000001a19661c117");
    assert!(is_newer(&accepted, None));
    assert!(!is_newer(&accepted, Some(&accepted)));
    assert!(is_newer(
        &version("2026-11-01T09:12:32Z-000001a19661c117"),
        Some(&accepted)
    ));
    assert!(!is_newer(
        &version("2026-11-01T09:12:30Z-ffffffffffffffff"),
        Some(&accepted)
    ));
}

#[test]
fn skew_bounds_the_future_always_and_the_past_only_at_first() {
    let now = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    let ahead = now + Duration::minutes(6);
    let behind = now - Duration::hours(3);
    assert!(dated_beyond_skew(ahead, now, true));
    assert!(dated_beyond_skew(ahead, now, false));
    assert!(!dated_beyond_skew(behind, now, true));
    assert!(dated_beyond_skew(behind, now, false));
    assert!(!dated_beyond_skew(now - Duration::minutes(4), now, false));
}
