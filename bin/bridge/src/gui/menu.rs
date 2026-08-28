//! Menu-bar construction and about-dialog metadata.
//!
//! macOS only. On Windows the same commands live in the web topbar overflow
//! menu: a `muda` menu bar renders as a system-coloured Win32 strip between the
//! title bar and a near-black web UI, which is one chrome band too many.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::collections::HashMap;

use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

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

    menu.init_for_nsapp();

    Ok(MenuBarHandles { menu })
}

fn about_metadata() -> muda::AboutMetadata {
    muda::AboutMetadata {
        name: Some(crate::brand::brand().app_name.into()),
        version: Some(crate::brand::brand().version.into()),
        ..Default::default()
    }
}
