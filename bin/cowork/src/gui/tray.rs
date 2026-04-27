use std::collections::HashMap;

use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::events::UiEvent;
use super::state::{AppStateSnapshot, GatewayStatus};

pub struct TrayHandles {
    pub _tray: TrayIcon,
    pub menu: Menu,
    pub bindings: HashMap<MenuId, UiEvent>,
    pub identity_item: MenuItem,
    pub last_sync_item: MenuItem,
    pub sync_item: MenuItem,
}

const TRAY_ICON_PNG: &[u8] = include_bytes!("../../assets/tray-icon.png");

pub fn build(initial: &AppStateSnapshot) -> Result<TrayHandles, String> {
    let menu = Menu::new();

    let identity_item = MenuItem::new(format_identity(initial), false, None);
    let last_sync_item = MenuItem::new(format_last_sync(initial), false, None);
    let sync_item = MenuItem::new("Sync now", true, None);
    let validate_item = MenuItem::new("Validate setup", true, None);
    let open_settings_item = MenuItem::new("Open settings…", true, None);
    let open_folder_item = MenuItem::new("Open config folder", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    menu.append(&identity_item).map_err(|e| e.to_string())?;
    menu.append(&last_sync_item).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&sync_item).map_err(|e| e.to_string())?;
    menu.append(&validate_item).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&open_settings_item).map_err(|e| e.to_string())?;
    menu.append(&open_folder_item).map_err(|e| e.to_string())?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&quit_item).map_err(|e| e.to_string())?;

    let mut bindings = HashMap::new();
    bindings.insert(sync_item.id().clone(), UiEvent::SyncRequested);
    bindings.insert(validate_item.id().clone(), UiEvent::ValidateRequested);
    bindings.insert(open_settings_item.id().clone(), UiEvent::OpenSettings);
    bindings.insert(open_folder_item.id().clone(), UiEvent::OpenConfigFolder);
    bindings.insert(quit_item.id().clone(), UiEvent::Quit);

    let icon = decode_icon().map_err(|e| format!("tray icon: {e}"))?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_tooltip("systemprompt-cowork")
        .with_icon(icon)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(TrayHandles {
        _tray: tray,
        menu,
        bindings,
        identity_item,
        last_sync_item,
        sync_item,
    })
}

pub fn refresh(handles: &TrayHandles, snap: &AppStateSnapshot) {
    handles.identity_item.set_text(format_identity(snap));
    handles.last_sync_item.set_text(format_last_sync(snap));
    handles
        .sync_item
        .set_enabled(!snap.sync_in_flight);
    if snap.sync_in_flight {
        handles.sync_item.set_text("Syncing…");
    } else {
        handles.sync_item.set_text("Sync now");
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
        if matches!(event, TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }) {
            out.push(UiEvent::OpenSettings);
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

fn decode_icon() -> Result<Icon, String> {
    let img = image::load_from_memory(TRAY_ICON_PNG)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).map_err(|e| e.to_string())
}
