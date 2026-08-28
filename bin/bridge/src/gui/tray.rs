//! System-tray icon, tooltip, and identity display.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use muda::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::error::{GuiError, GuiResult};
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

// Why: the notification area draws at 16px. macOS wants the flat monochrome
// template; Windows wants the 16x16 frame the .ico already carries, not a
// 1024px app icon resampled down to a smudge. Split by `#[cfg]` rather than
// `cfg!`, which compiles both arms: `image`'s ICO decoder is a Windows-only
// feature of the dependency, so the macOS build cannot name it at all.
#[cfg(target_os = "macos")]
fn tray_image() -> Result<image::RgbaImage, image::ImageError> {
    let assets = crate::brand::brand().assets;
    Ok(image::load_from_memory(assets.tray_icon_png)?.to_rgba8())
}

#[cfg(target_os = "windows")]
fn tray_image() -> Result<image::RgbaImage, image::ImageError> {
    let assets = crate::brand::brand().assets;
    let reader = image::codecs::ico::IcoDecoder::new(std::io::Cursor::new(assets.app_icon_ico));
    match reader {
        Ok(decoder) => Ok(image::DynamicImage::from_decoder(decoder)?.to_rgba8()),
        Err(e) => {
            tracing::warn!(error = %e, "app icon ICO undecodable; falling back to the window icon");
            Ok(image::load_from_memory(assets.window_icon_png)?.to_rgba8())
        },
    }
}

pub fn build(initial: &AppStateSnapshot) -> GuiResult<TrayHandles> {
    let menu = Menu::new();

    let identity_item = MenuItem::new(format_identity(initial), false, None);
    let last_sync_item = MenuItem::new(format_last_sync(initial), false, None);
    let sync_item = MenuItem::new(i18n::t("tray-sync-now"), true, None);
    let validate_item = MenuItem::new(i18n::t("tray-validate"), true, None);
    let update_item = MenuItem::new(i18n::t("tray-check-updates"), true, None);
    let open_settings_item = MenuItem::new(i18n::t("tray-open-settings"), true, None);
    let open_folder_item = MenuItem::new(i18n::t("tray-open-config"), true, None);
    let autostart = crate::install::gui_autostart_status();
    let autostart_item = CheckMenuItem::new(
        i18n::t("tray-autostart"),
        autostart != ScheduleStatus::Unknown,
        autostart == ScheduleStatus::Installed,
        None,
    );
    let logout_item = MenuItem::new(i18n::t("tray-sign-out"), is_signed_in(initial), None);
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
    bindings.insert(
        update_item.id().clone(),
        UiEvent::UpdateCheckRequested { reply_to: None },
    );
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
    bindings.insert(quit_item.id().clone(), UiEvent::Quit);

    let icon_normal = decode_icon()?;
    let icon_alert = decode_alert_icon()?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_menu_on_left_click(false)
        .with_tooltip(tooltip(initial))
        .with_icon(icon_normal.clone())
        .with_icon_as_template(cfg!(target_os = "macos"))
        .build()?;

    Ok(TrayHandles {
        tray,
        menu,
        bindings,
        identity_item,
        last_sync_item,
        sync_item,
        autostart_item,
        logout_item,
        icon_normal,
        icon_alert,
        status: TrayStatus::Normal,
    })
}

pub fn refresh(handles: &mut TrayHandles, snap: &AppStateSnapshot) {
    handles.identity_item.set_text(format_identity(snap));
    handles.last_sync_item.set_text(format_last_sync(snap));
    handles.sync_item.set_enabled(!snap.sync_in_flight);
    handles.logout_item.set_enabled(is_signed_in(snap));
    if snap.sync_in_flight {
        handles.sync_item.set_text(i18n::t("tray-syncing"));
    } else {
        handles.sync_item.set_text(i18n::t("tray-sync-now"));
    }
    // Why: a tick box cannot say "I could not ask the scheduler". Greying it out
    // is the difference between a box the user has not ticked and one that will
    // silently refuse to tick.
    let autostart = crate::install::gui_autostart_status();
    handles
        .autostart_item
        .set_enabled(autostart != ScheduleStatus::Unknown);
    handles
        .autostart_item
        .set_checked(autostart == ScheduleStatus::Installed);
    _ = handles.tray.set_tooltip(Some(tooltip(snap)));
    let target = match snap.gateway_status {
        GatewayStatus::Unreachable { .. } => TrayStatus::Alert,
        _ => TrayStatus::Normal,
    };
    if target != handles.status {
        let icon = match target {
            TrayStatus::Normal => handles.icon_normal.clone(),
            TrayStatus::Alert => handles.icon_alert.clone(),
        };
        _ = handles.tray.set_icon(Some(icon));
        handles.status = target;
    }
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

fn tooltip(snap: &AppStateSnapshot) -> String {
    format!("{}\n{}", format_identity(snap), format_last_sync(snap))
}

const fn is_signed_in(snap: &AppStateSnapshot) -> bool {
    snap.pat_present || snap.verified_identity.is_some()
}

fn format_identity(snap: &AppStateSnapshot) -> String {
    match &snap.gateway_status {
        GatewayStatus::Unknown | GatewayStatus::Probing => "Checking gateway…".to_owned(),
        GatewayStatus::Unreachable { .. } => "Gateway unreachable".to_owned(),
        GatewayStatus::Reachable { .. } => match snap.verified_identity.as_ref() {
            Some(id) => {
                let label = id
                    .email
                    .as_deref()
                    .or_else(|| {
                        id.user_id
                            .as_ref()
                            .map(systemprompt_identifiers::UserId::as_str)
                    })
                    .unwrap_or("(verified)");
                format!("Signed in as {label}")
            },
            None if snap.pat_present => "PAT stored — verifying…".to_owned(),
            None => "Not signed in".to_owned(),
        },
    }
}

fn format_last_sync(snap: &AppStateSnapshot) -> String {
    snap.last_sync_summary.as_deref().map_or_else(
        || "Last sync: never".to_owned(),
        |s| format!("Last sync: {s}"),
    )
}

fn decode_icon() -> GuiResult<Icon> {
    let img = tray_image()?;
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).map_err(GuiError::from)
}

fn decode_alert_icon() -> GuiResult<Icon> {
    let mut img = tray_image()?;
    let (w, h) = img.dimensions();
    let dot_radius = (w.min(h) / 4).max(3);
    // Why: centring the dot on the corner pixel clipped half of it outside the
    // bitmap, which at 16px left an ambiguous smear rather than an alert.
    let cx = w.saturating_sub(dot_radius).saturating_sub(1);
    let cy = h.saturating_sub(dot_radius).saturating_sub(1);
    for y in 0..h {
        for x in 0..w {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            if dx * dx + dy * dy <= (dot_radius as i32).pow(2) {
                img.put_pixel(x, y, image::Rgba([220, 38, 38, 255]));
            }
        }
    }
    Icon::from_rgba(img.into_raw(), w, h).map_err(GuiError::from)
}
