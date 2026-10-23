//! Tray menu construction and the update-entry projection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use muda::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem};

use super::text;
use crate::gui::error::GuiResult;
use crate::gui::events::UiEvent;
use crate::gui::state::AppStateSnapshot;
use crate::i18n;
use crate::install::ScheduleStatus;
use crate::update::UpdateUiState;

pub(super) struct MenuParts {
    pub(super) menu: Menu,
    pub(super) bindings: HashMap<MenuId, UiEvent>,
    pub(super) identity_item: MenuItem,
    pub(super) last_sync_item: MenuItem,
    pub(super) sync_item: MenuItem,
    pub(super) update_item: MenuItem,
    pub(super) autostart_item: CheckMenuItem,
    pub(super) logout_item: MenuItem,
}

pub(super) fn build_menu(
    initial: &AppStateSnapshot,
    schedule: &crate::schedule::status::ScheduleStatusCache,
) -> GuiResult<MenuParts> {
    let menu = Menu::new();

    let identity_item = MenuItem::new(text::format_identity(initial), false, None);
    let last_sync_item = MenuItem::new(text::format_last_sync(initial), false, None);
    let sync_item = MenuItem::new(i18n::t("tray-sync-now"), true, None);
    let validate_item = MenuItem::new(i18n::t("tray-validate"), true, None);
    let (update_label, update_enabled, update_event) = update_menu(&initial.update);
    let update_item = MenuItem::new(update_label, update_enabled, None);
    let open_settings_item = MenuItem::new(i18n::t("tray-open-settings"), true, None);
    let open_folder_item = MenuItem::new(i18n::t("tray-open-config"), true, None);
    let autostart = crate::install::gui_autostart_status(schedule);
    let autostart_item = CheckMenuItem::new(
        i18n::t("tray-autostart"),
        autostart != ScheduleStatus::Unknown,
        autostart == ScheduleStatus::Installed,
        None,
    );
    let logout_item = MenuItem::new(i18n::t("tray-sign-out"), text::is_signed_in(initial), None);
    let device_items = device_action_items();
    let quit_item = MenuItem::new(i18n::t("tray-quit"), true, None);

    menu.append(&identity_item)?;
    menu.append(&last_sync_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&sync_item)?;
    menu.append(&validate_item)?;
    menu.append(&update_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&open_settings_item)?;
    menu.append(&open_folder_item)?;
    menu.append(&autostart_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&logout_item)?;
    for (item, _) in &device_items {
        menu.append(item)?;
    }
    menu.append(&quit_item)?;

    let mut bindings = HashMap::new();
    bindings.insert(
        sync_item.id().clone(),
        UiEvent::SyncRequested { reply_to: None },
    );
    bindings.insert(
        validate_item.id().clone(),
        UiEvent::ValidateRequested { reply_to: None },
    );
    bindings.insert(update_item.id().clone(), update_event);
    bindings.insert(
        autostart_item.id().clone(),
        UiEvent::AutostartToggleRequested,
    );
    bindings.insert(open_settings_item.id().clone(), UiEvent::OpenSettings);
    bindings.insert(open_folder_item.id().clone(), UiEvent::OpenConfigFolder);
    bindings.insert(
        logout_item.id().clone(),
        UiEvent::LogoutRequested { reply_to: None },
    );
    for (item, event) in device_items {
        bindings.insert(item.id().clone(), event);
    }
    bindings.insert(quit_item.id().clone(), UiEvent::Quit);

    Ok(MenuParts {
        menu,
        bindings,
        identity_item,
        last_sync_item,
        sync_item,
        update_item,
        autostart_item,
        logout_item,
    })
}

fn device_action_items() -> [(MenuItem, UiEvent); 3] {
    use crate::wire::DeviceAction;

    [
        (
            MenuItem::new(i18n::t("tray-disconnect"), true, None),
            UiEvent::OpenDeviceAction(DeviceAction::Disconnect),
        ),
        (
            MenuItem::new(i18n::t("tray-purge"), true, None),
            UiEvent::OpenDeviceAction(DeviceAction::Purge),
        ),
        (
            MenuItem::new(i18n::t("tray-remove-application"), true, None),
            UiEvent::OpenDeviceAction(DeviceAction::RemoveApplication),
        ),
    ]
}

pub fn update_menu(state: &UpdateUiState) -> (String, bool, UiEvent) {
    match state {
        UpdateUiState::Available { version, .. } => (
            i18n::t_args("tray-update-to", &[("version", version)]),
            true,
            UiEvent::UpdateInstallRequested { reply_to: None },
        ),
        UpdateUiState::Downloading { version, percent } => (
            i18n::t_args(
                "tray-update-downloading",
                &[("version", version), ("percent", &percent.to_string())],
            ),
            false,
            UiEvent::UpdateCheckRequested { reply_to: None },
        ),
        UpdateUiState::Installing { version } => (
            i18n::t_args("tray-update-installing", &[("version", version)]),
            false,
            UiEvent::UpdateCheckRequested { reply_to: None },
        ),
        UpdateUiState::Ready { .. } => (
            i18n::t("tray-restart-update"),
            true,
            UiEvent::UpdateRestartRequested,
        ),
        UpdateUiState::Failed { .. } => (
            i18n::t("tray-update-check-failed"),
            true,
            UiEvent::UpdateCheckRequested { reply_to: None },
        ),
        UpdateUiState::Unknown | UpdateUiState::Current => (
            i18n::t("tray-check-updates"),
            true,
            UiEvent::UpdateCheckRequested { reply_to: None },
        ),
    }
}
