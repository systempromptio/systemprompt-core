use systemprompt_bridge::update::{UpdateStatus, UpdateUiState};
use systemprompt_bridge::verdict::Tone;

#[test]
fn default_ui_state_is_unknown_with_unknown_tone() {
    let state = UpdateUiState::default();
    assert!(matches!(state, UpdateUiState::Unknown));
    assert_eq!(state.tone(), Tone::Unknown);
    assert_eq!(state.version(), None);
}

#[test]
fn tone_maps_each_phase() {
    assert_eq!(UpdateUiState::Current.tone(), Tone::Ok);
    assert_eq!(
        UpdateUiState::Available {
            version: "1.0.0".to_owned(),
            notes_url: None
        }
        .tone(),
        Tone::Warn
    );
    assert_eq!(
        UpdateUiState::Ready {
            version: "1.0.0".to_owned()
        }
        .tone(),
        Tone::Warn
    );
    assert_eq!(
        UpdateUiState::Downloading {
            version: "1.0.0".to_owned(),
            percent: 10
        }
        .tone(),
        Tone::Probing
    );
    assert_eq!(
        UpdateUiState::Installing {
            version: "1.0.0".to_owned()
        }
        .tone(),
        Tone::Probing
    );
    assert_eq!(
        UpdateUiState::Failed {
            message: "boom".to_owned()
        }
        .tone(),
        Tone::Err
    );
}

#[test]
fn only_available_can_install_and_only_ready_can_restart() {
    let available = UpdateUiState::Available {
        version: "2.0.0".to_owned(),
        notes_url: None,
    };
    let ready = UpdateUiState::Ready {
        version: "2.0.0".to_owned(),
    };
    assert!(available.can_install());
    assert!(!available.can_restart());
    assert!(ready.can_restart());
    assert!(!ready.can_install());
    assert!(!UpdateUiState::Current.can_install());
    assert!(!UpdateUiState::Current.can_restart());
}

#[test]
fn in_progress_is_only_downloading_and_installing() {
    assert!(
        UpdateUiState::Downloading {
            version: "2.0.0".to_owned(),
            percent: 0
        }
        .in_progress()
    );
    assert!(
        UpdateUiState::Installing {
            version: "2.0.0".to_owned()
        }
        .in_progress()
    );
    assert!(!UpdateUiState::Unknown.in_progress());
    assert!(!UpdateUiState::Current.in_progress());
    assert!(
        !UpdateUiState::Ready {
            version: "2.0.0".to_owned()
        }
        .in_progress()
    );
    assert!(
        !UpdateUiState::Failed {
            message: "x".to_owned()
        }
        .in_progress()
    );
}

#[test]
fn version_is_carried_by_every_versioned_phase() {
    assert_eq!(
        UpdateUiState::Available {
            version: "3.1.0".to_owned(),
            notes_url: Some("https://example.test/notes".to_owned())
        }
        .version(),
        Some("3.1.0")
    );
    assert_eq!(
        UpdateUiState::Downloading {
            version: "3.1.1".to_owned(),
            percent: 50
        }
        .version(),
        Some("3.1.1")
    );
    assert_eq!(
        UpdateUiState::Installing {
            version: "3.1.2".to_owned()
        }
        .version(),
        Some("3.1.2")
    );
    assert_eq!(
        UpdateUiState::Ready {
            version: "3.1.3".to_owned()
        }
        .version(),
        Some("3.1.3")
    );
    assert_eq!(UpdateUiState::Current.version(), None);
    assert_eq!(
        UpdateUiState::Failed {
            message: "3.1.4".to_owned()
        }
        .version(),
        None
    );
}

#[test]
fn available_status_converts_to_available_ui_state_with_notes() {
    let status = UpdateStatus::Available {
        version: "4.0.0".to_owned(),
        notes_url: Some("https://example.test/rel".to_owned()),
    };
    let ui = UpdateUiState::from(&status);
    match ui {
        UpdateUiState::Available { version, notes_url } => {
            assert_eq!(version, "4.0.0");
            assert_eq!(notes_url.as_deref(), Some("https://example.test/rel"));
        },
        other => panic!("expected Available, got {other:?}"),
    }
}

#[test]
fn current_status_converts_to_current_ui_state() {
    let status = UpdateStatus::Current {
        version: "4.0.0".to_owned(),
    };
    assert!(matches!(
        UpdateUiState::from(&status),
        UpdateUiState::Current
    ));
}

#[test]
fn ui_state_serialises_with_kebab_phase_tag() {
    let json = serde_json::to_value(UpdateUiState::Downloading {
        version: "5.0.0".to_owned(),
        percent: 42,
    })
    .expect("serialise");
    assert_eq!(json["phase"], "downloading");
    assert_eq!(json["version"], "5.0.0");
    assert_eq!(json["percent"], 42);

    let unknown = serde_json::to_value(UpdateUiState::Unknown).expect("serialise");
    assert_eq!(unknown["phase"], "unknown");
}

#[test]
fn available_ui_state_omits_absent_notes_url() {
    let json = serde_json::to_value(UpdateUiState::Available {
        version: "5.1.0".to_owned(),
        notes_url: None,
    })
    .expect("serialise");
    assert_eq!(json["phase"], "available");
    assert!(json.get("notes_url").is_none());
}

#[test]
fn ui_state_round_trips_through_json() {
    let original = UpdateUiState::Ready {
        version: "6.0.0".to_owned(),
    };
    let text = serde_json::to_string(&original).expect("serialise");
    let back: UpdateUiState = serde_json::from_str(&text).expect("deserialise");
    assert_eq!(back.version(), Some("6.0.0"));
    assert!(back.can_restart());
}
