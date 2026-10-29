use semver::Version;
use systemprompt_models::bridge::instance::{BridgeVersionWindow, WindowStatus};

fn window(min: Option<&str>, max: Option<&str>) -> BridgeVersionWindow {
    BridgeVersionWindow {
        min: min.map(|v| Version::parse(v).expect("min")),
        max: max.map(|v| Version::parse(v).expect("max")),
        enforce_max: false,
    }
}

#[test]
fn an_open_window_admits_every_version_and_reports_within() {
    let open = BridgeVersionWindow::default();
    assert!(open.is_open());
    assert!(open.admits(&Version::new(0, 1, 0)));
    assert_eq!(open.status("9.9.9"), WindowStatus::Within);
}

#[test]
fn bounds_are_inclusive() {
    let w = window(Some("0.74.0"), Some("0.75.3"));
    assert!(w.admits(&Version::new(0, 74, 0)));
    assert!(w.admits(&Version::new(0, 75, 3)));
    assert!(!w.admits(&Version::new(0, 73, 9)));
    assert!(!w.admits(&Version::new(0, 75, 4)));
}

#[test]
fn status_places_a_reported_release_against_the_window() {
    let w = window(Some("0.74.0"), Some("0.75.3"));
    assert_eq!(w.status("0.73.9"), WindowStatus::Below);
    assert_eq!(w.status("0.75.0"), WindowStatus::Within);
    assert_eq!(w.status("0.76.0"), WindowStatus::Above);
    assert_eq!(w.status("not-a-version"), WindowStatus::Unknown);
}

#[test]
fn build_metadata_does_not_move_a_version_across_a_bound() {
    let w = window(None, Some("0.75.0"));
    assert_eq!(w.status("0.75.0+dynapps.9"), WindowStatus::Within);
    assert!(w.admits(&Version::parse("0.75.0+dynapps.9").expect("version")));
}

#[test]
fn a_prerelease_sorts_below_its_release() {
    let w = window(Some("0.75.0"), None);
    assert_eq!(w.status("0.75.0-rc.1"), WindowStatus::Below);
}

#[test]
fn intersect_takes_the_tighter_bound_on_each_side() {
    let a = window(Some("0.74.0"), Some("0.76.0"));
    let mut b = window(Some("0.75.0"), None);
    b.enforce_max = true;
    let both = a.intersect(&b);
    assert_eq!(both.min, Some(Version::new(0, 75, 0)));
    assert_eq!(both.max, Some(Version::new(0, 76, 0)));
    assert!(both.enforce_max);
    assert!(!both.is_empty());
}

#[test]
fn disjoint_windows_intersect_to_an_empty_window() {
    let both = window(Some("0.74.0"), Some("0.74.9")).intersect(&window(Some("0.75.0"), None));
    assert!(both.is_empty());
    assert!(!both.admits(&Version::new(0, 74, 5)));
}

#[test]
fn the_window_serialises_without_absent_bounds() {
    let json = serde_json::to_value(window(Some("0.74.0"), None)).expect("serialise");
    assert_eq!(
        json,
        serde_json::json!({ "min": "0.74.0", "enforce_max": false })
    );
}

#[test]
fn window_status_wire_names_are_snake_case() {
    assert_eq!(
        serde_json::to_value(WindowStatus::Within).expect("serialise"),
        serde_json::json!("within")
    );
}
