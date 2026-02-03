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
    let mut dir_path = base_path.join(&playbook.category);

    let domain_parts: Vec<&str> = playbook.domain.split('/').collect();

    for part in domain_parts
        .iter()
        .take(domain_parts.len().saturating_sub(1))
    {
        dir_path = dir_path.join(part);
    }

    fs::create_dir_all(&dir_path)?;

    let filename = domain_parts.last().unwrap_or(&"playbook");
    let file_path = dir_path.join(format!("{}.md", filename));

    let content = generate_playbook_markdown(playbook);
    fs::write(file_path, content)?;

    Ok(())
}
