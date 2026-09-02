use systemprompt_bridge::update::{UpdateStatus, platform_slug};

#[test]
fn platform_slug_matches_the_host_target() {
    let slug = platform_slug();
    if cfg!(target_os = "macos") {
        assert_eq!(slug, Some("macos"));
    } else if cfg!(target_os = "windows") {
        assert_eq!(slug, Some("windows"));
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86_64") {
            assert_eq!(slug, Some("linux-x86_64"));
        } else if cfg!(target_arch = "aarch64") {
            assert_eq!(slug, Some("linux-aarch64"));
        } else {
            assert_eq!(slug, None);
        }
    }
}

#[test]
fn platform_slug_never_contains_a_path_separator() {
    if let Some(slug) = platform_slug() {
        assert!(!slug.contains('/'), "slug {slug} would escape the URL path");
        assert!(!slug.is_empty());
    }
}

#[test]
fn is_available_distinguishes_the_two_states() {
    assert!(
        UpdateStatus::Available {
            version: "1.2.3".to_owned(),
            notes_url: None
        }
        .is_available()
    );
    assert!(
        !UpdateStatus::Current {
            version: "1.2.3".to_owned()
        }
        .is_available()
    );
}

#[test]
fn status_serialises_with_snake_case_state_tag() {
    let current = serde_json::to_value(UpdateStatus::Current {
        version: "1.2.3".to_owned(),
    })
    .expect("serialise");
    assert_eq!(current["state"], "current");
    assert_eq!(current["version"], "1.2.3");

    let available = serde_json::to_value(UpdateStatus::Available {
        version: "1.3.0".to_owned(),
        notes_url: Some("https://example.test/n".to_owned()),
    })
    .expect("serialise");
    assert_eq!(available["state"], "available");
    assert_eq!(available["notes_url"], "https://example.test/n");
}

#[test]
fn available_status_omits_absent_notes_url() {
    let json = serde_json::to_value(UpdateStatus::Available {
        version: "1.3.0".to_owned(),
        notes_url: None,
    })
    .expect("serialise");
    assert!(json.get("notes_url").is_none());
}

#[test]
fn equal_statuses_compare_equal_and_differing_versions_do_not() {
    let a = UpdateStatus::Current {
        version: "1.0.0".to_owned(),
    };
    let b = UpdateStatus::Current {
        version: "1.0.0".to_owned(),
    };
    let c = UpdateStatus::Current {
        version: "1.0.1".to_owned(),
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}
