use super::escape_yaml;
use anyhow::Result;
use std::fs;
use std::path::Path;
use systemprompt_agent::models::Playbook;

pub fn generate_playbook_markdown(playbook: &Playbook) -> String {
    let tags_yaml = if playbook.tags.is_empty() {
        "[]".to_string()
    } else {
        playbook
            .tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"---
title: "{}"
slug: "{}"
description: "{}"
enabled: {}
keywords:
{}
---

{}"#,
        escape_yaml(&playbook.name),
        playbook.playbook_id.as_str(),
        escape_yaml(&playbook.description),
        playbook.enabled,
        tags_yaml,
        &playbook.instructions
    )
}

pub fn export_playbook_to_disk(playbook: &Playbook, base_path: &Path) -> Result<()> {
    let category_dir = base_path.join(&playbook.category);
    fs::create_dir_all(&category_dir)?;

    let file_name = format!("{}.md", playbook.domain);
    let file_path = category_dir.join(&file_name);

    let content = generate_playbook_markdown(playbook);
    fs::write(file_path, content)?;

    Ok(())
}
