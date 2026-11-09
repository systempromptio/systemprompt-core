use systemprompt_bridge::update::{StagedUpdate, UpdateStatus, UpdateUiState};

fn ready(version: &str) -> UpdateUiState {
    UpdateUiState::Ready {
        version: version.to_owned(),
    }
}

fn version_of(state: &UpdateUiState) -> Option<String> {
    state.version().map(str::to_owned)
}

#[test]
fn a_staged_status_shows_as_ready_to_restart() {
    let state = UpdateUiState::from(&UpdateStatus::Staged {
        version: "0.79.0".to_owned(),
    });
    assert!(state.can_restart());
    assert!(!state.can_install());
    assert_eq!(version_of(&state).as_deref(), Some("0.79.0"));
}

#[test]
fn a_staged_record_seeds_the_state_as_ready() {
    let seeded = UpdateUiState::seeded(Some(&StagedUpdate {
        version: "0.79.0".to_owned(),
        program: std::path::PathBuf::from("bridge"),
        sha256: "0".repeat(64),
    }));
    assert!(seeded.can_restart());
    assert!(matches!(
        UpdateUiState::seeded(None),
        UpdateUiState::Unknown
    ));
}

#[test]
fn ready_survives_a_check_that_carries_no_information() {
    let after_failure = ready("0.79.0").after_check(UpdateUiState::Failed {
        message: "offline".to_owned(),
    });
    assert!(after_failure.can_restart());
    assert!(
        ready("0.79.0")
            .after_check(UpdateUiState::Unknown)
            .can_restart()
    );
}

#[test]
fn a_definitive_check_replaces_ready() {
    assert!(matches!(
        ready("0.79.0").after_check(UpdateUiState::Current),
        UpdateUiState::Current
    ));
    let same = ready("0.79.0").after_check(UpdateUiState::Available {
        version: "0.79.0".to_owned(),
        notes_url: None,
    });
    assert!(
        same.can_install(),
        "a record that no longer holds is offered again"
    );
    let older = ready("0.79.0").after_check(UpdateUiState::Available {
        version: "0.78.0".to_owned(),
        notes_url: None,
    });
    assert_eq!(version_of(&older).as_deref(), Some("0.78.0"));
    let newer = ready("0.79.0").after_check(ready("0.80.0"));
    assert_eq!(version_of(&newer).as_deref(), Some("0.80.0"));
    assert!(newer.can_restart());
}

#[test]
fn a_state_without_a_staged_release_takes_the_check_result() {
    let after = UpdateUiState::Unknown.after_check(UpdateUiState::Failed {
        message: "offline".to_owned(),
    });
    assert!(matches!(after, UpdateUiState::Failed { .. }));
}

#[test]
fn a_download_in_progress_survives_a_check_result() {
    let downloading = UpdateUiState::Downloading {
        version: "0.80.0".to_owned(),
        percent: 40,
    };
    let after = downloading.after_check(UpdateUiState::Available {
        version: "0.80.0".to_owned(),
        notes_url: None,
    });
    assert!(after.in_progress());
    assert!(
        !after.can_install(),
        "a running download cannot be started twice"
    );
    assert!(
        downloading
            .after_check(UpdateUiState::Current)
            .in_progress()
    );
}
