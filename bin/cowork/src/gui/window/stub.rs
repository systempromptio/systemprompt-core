use std::sync::mpsc::Sender;

use crate::gui::events::UiEvent;
use crate::gui::state::AppStateSnapshot;

pub struct PlatformWindow;

impl PlatformWindow {
    pub fn new(_tx: Sender<UiEvent>) -> Result<Self, String> {
        Err("settings window is not supported on this platform".into())
    }
    pub fn show(&self, _snapshot: &AppStateSnapshot) {}
    pub fn refresh(&self, _snapshot: &AppStateSnapshot) {}
    pub fn close(&self) {}
}
