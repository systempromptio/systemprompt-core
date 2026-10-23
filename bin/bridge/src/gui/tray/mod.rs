//! System-tray icon, tooltip, and identity display.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use muda::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

mod icon;
mod menu;
mod text;

pub use self::menu::update_menu;
use self::menu::{MenuParts, build_menu};
use super::error::GuiResult;
use super::events::UiEvent;
use super::state::{AppStateSnapshot, GatewayStatus};
use crate::i18n;
use crate::install::ScheduleStatus;

pub struct TrayHandles {
    pub tray: TrayIcon,
    pub menu: Menu,
    pub bindings: HashMap<MenuId, UiEvent>,
    pub identity_item: MenuItem,
    pub last_sync_item: MenuItem,
    pub sync_item: MenuItem,
    pub update_item: MenuItem,
    pub autostart_item: CheckMenuItem,
    pub logout_item: MenuItem,
    pub icon_normal: Icon,
    pub icon_alert: Icon,
    pub status: TrayStatus,
}

impl std::fmt::Debug for TrayHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayHandles")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    Normal,
    Alert,
}

pub fn build(
    initial: &AppStateSnapshot,
    schedule: &crate::schedule::status::ScheduleStatusCache,
) -> GuiResult<TrayHandles> {
    let MenuParts {
        menu,
        bindings,
        identity_item,
        last_sync_item,
        sync_item,
        update_item,
        autostart_item,
        logout_item,
    } = build_menu(initial, schedule)?;

    let icon_normal = icon::decode_icon()?;
    let icon_alert = icon::decode_alert_icon()?;

    let builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_menu_on_left_click(false)
        .with_tooltip(text::tooltip(initial));
    #[cfg(target_os = "macos")]
    let builder = builder.with_icon_templated(icon_normal.clone());
    #[cfg(not(target_os = "macos"))]
    let builder = builder.with_icon(icon_normal.clone());
    let tray = builder.build()?;

    Ok(TrayHandles {
        tray,
        menu,
        bindings,
        identity_item,
        last_sync_item,
        sync_item,
        update_item,
        autostart_item,
        logout_item,
        icon_normal,
        icon_alert,
        status: TrayStatus::Normal,
    })
}

pub fn refresh(
    handles: &mut TrayHandles,
    snap: &AppStateSnapshot,
    schedule: &crate::schedule::status::ScheduleStatusCache,
) -> Result<(), tray_icon::Error> {
    handles.identity_item.set_text(text::format_identity(snap));
    handles
        .last_sync_item
        .set_text(text::format_last_sync(snap));
    handles.sync_item.set_enabled(!snap.sync_in_flight);
    let (update_label, update_enabled, update_event) = update_menu(&snap.update);
    handles.update_item.set_text(update_label);
    handles.update_item.set_enabled(update_enabled);
    handles
        .bindings
        .insert(handles.update_item.id().clone(), update_event);
    handles.logout_item.set_enabled(text::is_signed_in(snap));
    if snap.sync_in_flight {
        handles.sync_item.set_text(i18n::t("tray-syncing"));
    } else {
        handles.sync_item.set_text(i18n::t("tray-sync-now"));
    }
    let autostart = crate::install::gui_autostart_status(schedule);
    handles
        .autostart_item
        .set_enabled(autostart != ScheduleStatus::Unknown);
    handles
        .autostart_item
        .set_checked(autostart == ScheduleStatus::Installed);
    handles.tray.set_tooltip(Some(text::tooltip(snap)))?;
    let target = match snap.gateway_status {
        GatewayStatus::Unreachable { .. } => TrayStatus::Alert,
        _ => TrayStatus::Normal,
    };
    if target != handles.status {
        let icon = match target {
            TrayStatus::Normal => handles.icon_normal.clone(),
            TrayStatus::Alert => handles.icon_alert.clone(),
        };
        #[cfg(target_os = "macos")]
        handles.tray.set_icon_templated(Some(icon))?;
        #[cfg(not(target_os = "macos"))]
        handles.tray.set_icon(Some(icon))?;
        handles.status = target;
    }
    Ok(())
}

pub fn drain(handles: &TrayHandles) -> Vec<UiEvent> {
    let mut out = Vec::new();
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if let Some(ev) = handles.bindings.get(&event.id) {
            out.push(ev.clone());
        }
    }
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => out.push(UiEvent::OpenSettings),
            _ => {},
        }
    }
    out
}
