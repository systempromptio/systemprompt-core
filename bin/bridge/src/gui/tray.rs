use std::collections::HashMap;

use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::error::{GuiError, GuiResult};
use super::events::UiEvent;
use super::state::{AppStateSnapshot, GatewayStatus};

pub struct TrayHandles {
    pub tray: TrayIcon,
    pub menu: Menu,
    pub bindings: HashMap<MenuId, UiEvent>,
    pub identity_item: MenuItem,
    pub last_sync_item: MenuItem,
    pub sync_item: MenuItem,
    pub icon_normal: Icon,
    pub icon_alert: Icon,
    pub status: TrayStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    Normal,
    Alert,
}

#[cfg(target_os = "macos")]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/tray-icon.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/window-icon-1024.png");

pub fn build(initial: &AppStateSnapshot) -> GuiResult<TrayHandles> {
    let menu = Menu::new();

    let identity_item = MenuItem::new(format_identity(initial), false, None);
    let last_sync_item = MenuItem::new(format_last_sync(initial), false, None);
    let sync_item = MenuItem::new("Sync now", true, None);
    let validate_item = MenuItem::new("Validate setup", true, None);
    let open_settings_item = MenuItem::new("Open settings…", true, None);
    let open_folder_item = MenuItem::new("Open config folder", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    menu.append(&identity_item)?;
    menu.append(&last_sync_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&sync_item)?;
    menu.append(&validate_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&open_settings_item)?;
    menu.append(&open_folder_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
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
    bindings.insert(open_settings_item.id().clone(), UiEvent::OpenSettings);
    bindings.insert(open_folder_item.id().clone(), UiEvent::OpenConfigFolder);
    bindings.insert(quit_item.id().clone(), UiEvent::Quit);

    let icon_normal = decode_icon()?;
    let icon_alert = decode_alert_icon()?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_menu_on_left_click(false)
        .with_tooltip("systemprompt-bridge")
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
        icon_normal,
        icon_alert,
        status: TrayStatus::Normal,
    })
}

pub fn refresh(handles: &mut TrayHandles, snap: &AppStateSnapshot) {
    handles.identity_item.set_text(format_identity(snap));
    handles.last_sync_item.set_text(format_last_sync(snap));
    handles.sync_item.set_enabled(!snap.sync_in_flight);
    if snap.sync_in_flight {
        handles.sync_item.set_text("Syncing…");
    } else {
        handles.sync_item.set_text("Sync now");
    }
    let target = match snap.gateway_status {
        GatewayStatus::Unreachable { .. } => TrayStatus::Alert,
        _ => TrayStatus::Normal,
    };
    if target != handles.status {
        let icon = match target {
            TrayStatus::Normal => handles.icon_normal.clone(),
            TrayStatus::Alert => handles.icon_alert.clone(),
        };
        let _ = handles.tray.set_icon(Some(icon));
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

fn format_identity(snap: &AppStateSnapshot) -> String {
    match &snap.gateway_status {
        GatewayStatus::Unknown | GatewayStatus::Probing => "Checking gateway…".to_string(),
        GatewayStatus::Unreachable { .. } => "Gateway unreachable".to_string(),
        GatewayStatus::Reachable { .. } => match snap.verified_identity.as_ref() {
            Some(id) => {
                let label = id
                    .email
                    .as_deref()
                    .or(id.user_id.as_deref())
                    .unwrap_or("(verified)");
                format!("Signed in as {label}")
            },
            None if snap.pat_present => "PAT stored — verifying…".to_string(),
            None => "Not signed in".to_string(),
        },
    }
}

fn format_last_sync(snap: &AppStateSnapshot) -> String {
    match snap.last_sync_summary.as_deref() {
        Some(s) => format!("Last sync: {s}"),
        None => "Last sync: never".to_string(),
    }
}

fn decode_icon() -> GuiResult<Icon> {
    let img = image::load_from_memory(TRAY_ICON_PNG)?.to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).map_err(GuiError::from)
}

fn decode_alert_icon() -> GuiResult<Icon> {
    let mut img = image::load_from_memory(TRAY_ICON_PNG)?.to_rgba8();
    let (w, h) = img.dimensions();
    let dot_radius = (w.min(h) / 4).max(3);
    let cx = w.saturating_sub(dot_radius);
    let cy = h.saturating_sub(dot_radius);
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
