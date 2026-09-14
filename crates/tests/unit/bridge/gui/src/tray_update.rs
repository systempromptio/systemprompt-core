use systemprompt_bridge::gui::events::UiEvent;
use systemprompt_bridge::gui::tray::update_menu;
use systemprompt_bridge::update::UpdateUiState;

#[test]
fn available_release_turns_the_tray_action_into_install() {
    let (label, enabled, event) = update_menu(&UpdateUiState::Available {
        version: "0.52.0".to_owned(),
        notes_url: None,
    });
    assert_eq!(label, "Update to v0.52.0");
    assert!(enabled);
    assert!(matches!(event, UiEvent::UpdateInstallRequested { .. }));
}

#[test]
fn staged_release_turns_the_tray_action_into_restart() {
    let (label, enabled, event) = update_menu(&UpdateUiState::Ready {
        version: "0.52.0".to_owned(),
    });
    assert_eq!(label, "Restart to finish updating");
    assert!(enabled);
    assert!(matches!(event, UiEvent::UpdateRestartRequested));
}

#[test]
fn download_progress_disables_the_tray_action() {
    let (label, enabled, _) = update_menu(&UpdateUiState::Downloading {
        version: "0.52.0".to_owned(),
        percent: 37,
    });
    assert_eq!(label, "Downloading v0.52.0… 37%");
    assert!(!enabled);
}
