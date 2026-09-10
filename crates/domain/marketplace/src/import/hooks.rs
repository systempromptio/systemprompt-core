//! `plugins/<id>/hooks/hooks.json` → one `hooks/<id>__<event>__<n>/config.yaml`
//! per matcher/action pair.
//!
//! Claude Code groups hooks by event and matcher with several actions under
//! each; the loader's hook catalogue is flat, one directory per command. The
//! importer therefore expands the cross product and names each directory after
//! the plugin that shipped it so two plugins binding the same event cannot
//! collide.
//!
//! Only `command` actions survive: a prompt- or agent-typed action has no
//! command to run and no slot in the flat descriptor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::HookId;
use systemprompt_models::services::hooks::{HookCategory, HookEvent, HookType};

use crate::error::MarketplaceError;

use super::anthropic::HooksFile;
use super::disk::HookDoc;
use super::warning::ImportWarning;
use super::writer::Sink;

pub(super) struct ImportedHooks {
    pub ids: Vec<String>,
    pub warnings: Vec<ImportWarning>,
}

pub(super) fn import_plugin_hooks(
    plugin_id: &str,
    plugin_dir: &Path,
    sink: &Sink,
) -> Result<ImportedHooks, MarketplaceError> {
    let path = plugin_dir.join("hooks").join("hooks.json");
    let mut out = ImportedHooks {
        ids: Vec::new(),
        warnings: Vec::new(),
    };
    if !path.is_file() {
        return Ok(out);
    }

    let text = std::fs::read_to_string(&path).map_err(|e| MarketplaceError::Import {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    let file: HooksFile = serde_json::from_str(&text).map_err(|e| MarketplaceError::Import {
        path: path.display().to_string(),
        message: format!("hooks.json is not valid: {e}"),
    })?;

    for event in HookEvent::ALL_VARIANTS {
        let mut index = 0usize;
        for matcher in file.hooks.matchers_for_event(*event) {
            for action in &matcher.hooks {
                let Some(command) = command_of(action) else {
                    out.warnings.push(ImportWarning::UnsupportedHookAction {
                        plugin: plugin_id.to_owned(),
                        event: event.as_str().to_owned(),
                    });
                    continue;
                };
                let dir_name = format!("{plugin_id}__{}__{index}", event.as_str());
                let doc = HookDoc {
                    id: HookId::new(dir_name.replace('-', "_")),
                    name: format!("{plugin_id} {} {index}", event.as_str()),
                    description: String::new(),
                    version: "1.0.0".to_owned(),
                    enabled: true,
                    event: *event,
                    matcher: matcher.matcher.clone(),
                    command,
                    is_async: action.r#async,
                    category: HookCategory::Custom,
                    tags: vec![plugin_id.to_owned()],
                };
                let rel = Path::new("hooks").join(&dir_name).join("config.yaml");
                sink.write_yaml(&rel, &doc)?;
                out.ids.push(doc.id.as_str().to_owned());
                index += 1;
            }
        }
    }

    Ok(out)
}

fn command_of(action: &systemprompt_models::services::hooks::HookAction) -> Option<String> {
    match action.hook_type {
        HookType::Command => action.command.clone().filter(|c| !c.trim().is_empty()),
        HookType::Prompt | HookType::Agent => None,
    }
}
