//! Menu-bar construction and about-dialog metadata.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
#[cfg(target_os = "windows")]
use winit::window::Window;

use super::error::GuiResult;
use super::events::UiEvent;
use crate::i18n;

pub struct MenuBarHandles {
    pub menu: Menu,
}

impl std::fmt::Debug for MenuBarHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuBarHandles").finish_non_exhaustive()
    }
}

pub fn install<S: std::hash::BuildHasher>(
    bindings: &mut HashMap<MenuId, UiEvent, S>,
) -> GuiResult<MenuBarHandles> {
    let menu = Menu::new();

    #[cfg(target_os = "macos")]
    {
        let app_menu = Submenu::new(crate::brand::brand().app_menu_name, true);
        let about = PredefinedMenuItem::about(None, Some(about_metadata()));
        app_menu.append(&about)?;
        app_menu.append(&PredefinedMenuItem::separator())?;
        app_menu.append(&PredefinedMenuItem::hide(None))?;
        app_menu.append(&PredefinedMenuItem::hide_others(None))?;
        app_menu.append(&PredefinedMenuItem::separator())?;
        app_menu.append(&PredefinedMenuItem::quit(None))?;
        menu.append(&app_menu)?;
    }

    let edit_menu = Submenu::new(i18n::t("menu-edit"), true);
    edit_menu.append(&PredefinedMenuItem::undo(None))?;
    edit_menu.append(&PredefinedMenuItem::redo(None))?;
    edit_menu.append(&PredefinedMenuItem::separator())?;
    edit_menu.append(&PredefinedMenuItem::cut(None))?;
    edit_menu.append(&PredefinedMenuItem::copy(None))?;
    edit_menu.append(&PredefinedMenuItem::paste(None))?;
    edit_menu.append(&PredefinedMenuItem::select_all(None))?;
    menu.append(&edit_menu)?;

    let view_menu = Submenu::new(i18n::t("menu-view"), true);
    let show_settings = MenuItem::new(i18n::t("menu-show-settings"), true, None);
    bindings.insert(show_settings.id().clone(), UiEvent::FocusWindow);
    view_menu.append(&show_settings)?;
    menu.append(&view_menu)?;

    let help_menu = Submenu::new(i18n::t("menu-help"), true);
    let open_logs = MenuItem::new(i18n::t("menu-open-log-folder"), true, None);
    bindings.insert(
        open_logs.id().clone(),
        UiEvent::OpenLogDirectory { reply_to: None },
    );
    help_menu.append(&open_logs)?;

    let export_bundle = MenuItem::new(i18n::t("menu-export-bundle"), true, None);
    bindings.insert(
        export_bundle.id().clone(),
        UiEvent::ExportDiagnosticBundle { reply_to: None },
    );
    help_menu.append(&export_bundle)?;

    let open_config = MenuItem::new(i18n::t("menu-open-config"), true, None);
    bindings.insert(open_config.id().clone(), UiEvent::OpenConfigFolder);
    help_menu.append(&open_config)?;

    menu.append(&help_menu)?;

    #[cfg(target_os = "macos")]
    {
        menu.init_for_nsapp();
    }

    Ok(MenuBarHandles { menu })
}

#[cfg(target_os = "windows")]
#[expect(
    unsafe_code,
    reason = "raw window handle is required to attach Win32 menu bar to the GUI HWND"
)]
pub fn attach_to_window(handles: &MenuBarHandles, window: &Window) -> GuiResult<()> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().map_err(|e| {
        crate::gui::error::GuiError::Io(std::io::Error::other(format!(
            "window handle unavailable: {e}"
        )))
    })?;
    if let RawWindowHandle::Win32(w) = handle.as_raw() {
        // SAFETY: hwnd is a live HWND owned by GuiApp's settings Window; muda only
        // reads it.
        unsafe {
            handles.menu.init_for_hwnd(w.hwnd.get())?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn attach_to_window(
    _handles: &MenuBarHandles,
    _window: &winit::window::Window,
) -> GuiResult<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn about_metadata() -> muda::AboutMetadata {
    muda::AboutMetadata {
        name: Some(crate::brand::brand().app_name.into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    }
}
