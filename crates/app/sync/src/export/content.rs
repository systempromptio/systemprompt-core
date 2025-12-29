use super::escape_yaml;
use anyhow::Result;
use std::fs;
use std::path::Path;
use systemprompt_core_content::models::Content;

pub fn generate_content_markdown(content: &Content) -> String {
    let public_str = "true";
    let image_str = content.image.as_deref().unwrap_or("");
    let updated_at_str = content
        .updated_at
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    format!(
        r#"---
title: "{}"
description: "{}"
author: "{}"
slug: "{}"
keywords: "{}"
image: "{}"
kind: "{}"
public: {}
tags: []
published_at: "{}"
updated_at: "{}"
---

{}"#,
        escape_yaml(&content.title),
        escape_yaml(&content.description),
        escape_yaml(&content.author),
        &content.slug,
        escape_yaml(&content.keywords),
        image_str,
        &content.kind,
        public_str,
        content.published_at.format("%Y-%m-%d"),
        updated_at_str,
        &content.body
    )
}

pub fn export_content_to_file(
    content: &Content,
    base_path: &Path,
    source_type: &str,
) -> Result<()> {
    let markdown = generate_content_markdown(content);

    let content_dir = if source_type == "blog" {
        let dir = base_path.join(&content.slug);
        fs::create_dir_all(&dir)?;
        dir.join("index.md")
    } else {
        fs::create_dir_all(base_path)?;
        base_path.join(format!("{}.md", content.slug))
    };

    fs::write(&content_dir, markdown)?;
    Ok(())
}
