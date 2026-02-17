use anyhow::Result;
use std::path::Path;
use systemprompt_models::HookEventsConfig;

pub fn generate_hooks_json(
    hooks: &HookEventsConfig,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    if hooks.is_empty() {
        return Ok(());
    }

    let hooks_dir = output_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    let hooks_json = serde_json::to_value(hooks)?;
    let hooks_path = hooks_dir.join("hooks.json");
    let content = serde_json::to_string_pretty(&hooks_json)?;
    std::fs::write(&hooks_path, content)?;
    files_generated.push(hooks_path.to_string_lossy().to_string());

    Ok(())
}
