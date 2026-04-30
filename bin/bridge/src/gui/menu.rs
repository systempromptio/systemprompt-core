use std::collections::HashMap;

use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use super::error::GuiResult;
use super::events::UiEvent;

#[allow(dead_code)]
pub struct MenuBarHandles {
    pub menu: Menu,
}

pub fn install(bindings: &mut HashMap<MenuId, UiEvent>) -> GuiResult<MenuBarHandles> {
    let menu = Menu::new();

    #[cfg(target_os = "macos")]
    {
        let app_menu = Submenu::new("systemprompt-bridge", true);
        let about = PredefinedMenuItem::about(None, Some(about_metadata()));
        app_menu.append(&about)?;
        app_menu.append(&PredefinedMenuItem::separator())?;
        app_menu.append(&PredefinedMenuItem::hide(None))?;
        app_menu.append(&PredefinedMenuItem::hide_others(None))?;
        app_menu.append(&PredefinedMenuItem::separator())?;
        app_menu.append(&PredefinedMenuItem::quit(None))?;
        menu.append(&app_menu)?;
    }

    let edit_menu = Submenu::new("Edit", true);
    edit_menu.append(&PredefinedMenuItem::undo(None))?;
    edit_menu.append(&PredefinedMenuItem::redo(None))?;
    edit_menu.append(&PredefinedMenuItem::separator())?;
    edit_menu.append(&PredefinedMenuItem::cut(None))?;
    edit_menu.append(&PredefinedMenuItem::copy(None))?;
    edit_menu.append(&PredefinedMenuItem::paste(None))?;
    edit_menu.append(&PredefinedMenuItem::select_all(None))?;
    menu.append(&edit_menu)?;

    let view_menu = Submenu::new("View", true);
    let show_settings = MenuItem::new("Show settings", true, None);
    bindings.insert(show_settings.id().clone(), UiEvent::FocusWindow);
    view_menu.append(&show_settings)?;
    menu.append(&view_menu)?;

    let help_menu = Submenu::new("Help", true);
    let open_logs = MenuItem::new("Open log folder", true, None);
    bindings.insert(
        open_logs.id().clone(),
        UiEvent::OpenLogDirectory { reply_to: None },
    );
    help_menu.append(&open_logs)?;

    let export_bundle = MenuItem::new("Export diagnostic bundle…", true, None);
    bindings.insert(
        export_bundle.id().clone(),
        UiEvent::ExportDiagnosticBundle { reply_to: None },
    );
    help_menu.append(&export_bundle)?;

    let open_config = MenuItem::new("Open config folder", true, None);
    bindings.insert(open_config.id().clone(), UiEvent::OpenConfigFolder);
    help_menu.append(&open_config)?;

    menu.append(&help_menu)?;

    #[cfg(target_os = "macos")]
    {
        menu.init_for_nsapp();
    }

    Ok(MenuBarHandles { menu })
}

#[cfg(target_os = "macos")]
fn about_metadata() -> muda::AboutMetadata {
    muda::AboutMetadata {
        name: Some("systemprompt-bridge".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    }
}
