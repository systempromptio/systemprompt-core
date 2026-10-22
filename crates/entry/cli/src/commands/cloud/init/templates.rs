//! Default file contents used when scaffolding a new project.
//!
//! Each function returns the body of one generated service config, web
//! template, or starter content file written by `super::scaffolding`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Context;

pub fn root_config() -> String {
    r#"# systemprompt.io Services Configuration
settings:
  agent_port_range: [3100, 3199]
  mcp_port_range: [3200, 3299]
  auto_start_enabled: true
  validation_strict: false
  schema_validation_mode: "warn"
"#
    .to_owned()
}

pub fn agent_config(project_name: &str) -> String {
    format!(
        r#"# Assistant Agent Configuration
endpoint: assistant
port: 3100
enabled: true
default: true

card:
  display_name: "{} Assistant"
  description: "AI assistant powered by systemprompt.io"
  version: "1.0.0"

metadata:
  mcp_servers: []
  skills: []

prompt:
  system: |
    You are a helpful AI assistant.
"#,
        project_name
    )
}

pub fn admin_agent_config() -> String {
    r#"# Admin Agent Configuration
endpoint: admin
port: 3101
enabled: true
default: false

card:
  display_name: "Admin Agent"
  description: "Administrative agent for system management"
  version: "1.0.0"

metadata:
  mcp_servers:
    - systemprompt-admin
  skills: []

prompt:
  system: |
    You are an administrative assistant with access to system tools.
"#
    .to_owned()
}

pub fn admin_mcp_config() -> String {
    r#"# systemprompt.io Admin MCP Server
endpoint: systemprompt-admin
port: 3200
enabled: true
binary: "cargo"
path: "services/mcp/systemprompt-admin"
display_in_web: false

oauth:
  required: true
  scopes: ["admin"]
"#
    .to_owned()
}

pub fn ai_config() -> String {
    r"# AI policy: sampling, MCP discovery and history retention.
# The model catalog lives beside this file in credentials.yaml, providers.yaml,
# models.yaml, deployments.yaml and gateway.yaml.
ai: {}
"
    .to_owned()
}

/// The catalog files a new project starts with: core's default catalog with
/// `gateway.default_model` on `default_provider`, gateway off.
pub fn catalog_files(default_provider: &str) -> anyhow::Result<Vec<(&'static str, String)>> {
    let document = crate::commands::admin::setup::catalog::seed_document(
        crate::commands::admin::setup::catalog::SeedRequest {
            present: &["anthropic", "openai", "gemini"],
            primary: Some(default_provider),
            gateway_enabled: false,
        },
    )?;
    let section = |key: &str, value: serde_yaml::Value| -> anyhow::Result<String> {
        let mut map = serde_yaml::Mapping::new();
        map.insert(serde_yaml::Value::String(key.to_owned()), value);
        serde_yaml::to_string(&map).context("catalog section does not serialize")
    };
    Ok(vec![
        (
            "credentials.yaml",
            section("credentials", serde_yaml::to_value(&document.credentials)?)?,
        ),
        (
            "providers.yaml",
            section("providers", serde_yaml::to_value(&document.providers)?)?,
        ),
        (
            "models.yaml",
            section("models", serde_yaml::to_value(&document.models)?)?,
        ),
        (
            "deployments.yaml",
            section("deployments", serde_yaml::to_value(&document.deployments)?)?,
        ),
        (
            "gateway.yaml",
            section("gateway", serde_yaml::to_value(&document.gateway)?)?,
        ),
    ])
}

pub fn content_config() -> String {
    r#"# Content Configuration
# Define content sources for your project
# Example:
#   content_sources:
#     blog:
#       enabled: true
#       path: "content/blog"
#       source_id: "blog"
#       category_id: "articles"
#       description: "Blog posts"

content_sources: {}
"#
    .to_owned()
}

pub fn web_config(project_name: &str) -> String {
    format!(
        "# Web Configuration\nbranding:\n  site_name: \"{}\"\n  primary_color: \"#3b82f6\"\n",
        project_name
    )
}

pub fn web_metadata(project_name: &str) -> String {
    format!(
        r#"# Web Metadata
site:
  title: "{}"
  description: "Powered by systemprompt.io"
"#,
        project_name
    )
}

pub fn scheduler_config() -> String {
    r"# Scheduler Configuration
enabled: false
jobs: []
"
    .to_owned()
}

pub fn page_template() -> String {
    r"<!DOCTYPE html>
<html>
<head>
    <title>{{ title }}</title>
</head>
<body>
    <main>{{ content }}</main>
</body>
</html>
"
    .to_owned()
}

pub fn blog_post_template() -> String {
    r"<!DOCTYPE html>
<html>
<head>
    <title>{{ title }}</title>
</head>
<body>
    <article>
        <h1>{{ title }}</h1>
        <time>{{ date }}</time>
        <div>{{ content }}</div>
    </article>
</body>
</html>
"
    .to_owned()
}

pub fn blog_list_template() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <title>Blog</title>
</head>
<body>
    <h1>Blog</h1>
    <ul>
    {% for post in posts %}
        <li><a href="{{ post.url }}">{{ post.title }}</a></li>
    {% endfor %}
    </ul>
</body>
</html>
"#
    .to_owned()
}

pub fn page_list_template() -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <title>Pages</title>
</head>
<body>
    <h1>Pages</h1>
    <ul>
    {% for page in pages %}
        <li><a href="{{ page.url }}">{{ page.title }}</a></li>
    {% endfor %}
    </ul>
</body>
</html>
"#
    .to_owned()
}

pub fn welcome_blog_post(project_name: &str) -> String {
    format!(
        r"---
title: Welcome to {}
date: 2024-01-01
description: Getting started with your new project
---

# Welcome

This is your first blog post. Edit or delete this file to get started.
",
        project_name
    )
}

pub fn privacy_policy(project_name: &str) -> String {
    format!(
        r"---
title: Privacy Policy
---

# Privacy Policy

This is a placeholder privacy policy for {}.
",
        project_name
    )
}

pub fn cookie_policy(project_name: &str) -> String {
    format!(
        r"---
title: Cookie Policy
---

# Cookie Policy

This is a placeholder cookie policy for {}.
",
        project_name
    )
}
