//! Unit tests for the `cloud::init::templates` scaffolding content.
//!
//! Each generator returns the body of one file written during project
//! scaffolding. The assertions lock the structural anchors (YAML keys, template
//! placeholders, interpolated project names) that downstream loaders depend on.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::init::templates;

#[test]
fn root_config_declares_port_ranges_and_flags() {
    let out = templates::root_config();
    assert!(out.contains("agent_port_range: [3100, 3199]"));
    assert!(out.contains("mcp_port_range: [3200, 3299]"));
    assert!(out.contains("auto_start_enabled: true"));
    assert!(out.contains("schema_validation_mode: \"warn\""));
}

#[test]
fn agent_config_interpolates_project_name() {
    let out = templates::agent_config("Acme");
    assert!(out.contains("display_name: \"Acme Assistant\""));
    assert!(out.contains("endpoint: assistant"));
    assert!(out.contains("port: 3100"));
    assert!(out.contains("default: true"));
}

#[test]
fn admin_agent_config_references_admin_mcp_server() {
    let out = templates::admin_agent_config();
    assert!(out.contains("endpoint: admin"));
    assert!(out.contains("- systemprompt-admin"));
    assert!(out.contains("default: false"));
}

#[test]
fn admin_mcp_config_requires_oauth_admin_scope() {
    let out = templates::admin_mcp_config();
    assert!(out.contains("endpoint: systemprompt-admin"));
    assert!(out.contains("port: 3200"));
    assert!(out.contains("required: true"));
    assert!(out.contains("scopes: [\"admin\"]"));
}

#[test]
fn ai_config_sets_default_provider_and_provider_block() {
    let out = templates::ai_config("anthropic");
    assert!(out.contains("default_provider: \"anthropic\""));
    assert!(out.contains("anthropic:"));
    assert!(out.contains("openai:"));
    assert!(out.contains("gemini:"));
    assert!(out.contains("default_model:"));
}

#[test]
fn ai_config_honours_a_non_default_provider() {
    let out = templates::ai_config("gemini");
    assert!(out.contains("default_provider: \"gemini\""));
}

#[test]
fn content_config_starts_with_empty_sources_map() {
    let out = templates::content_config();
    assert!(out.contains("content_sources: {}"));
}

#[test]
fn web_config_and_metadata_carry_project_name() {
    let cfg = templates::web_config("Acme");
    assert!(cfg.contains("site_name: \"Acme\""));
    assert!(cfg.contains("primary_color: \"#3b82f6\""));

    let meta = templates::web_metadata("Acme");
    assert!(meta.contains("title: \"Acme\""));
    assert!(meta.contains("Powered by systemprompt.io"));
}

#[test]
fn scheduler_config_is_disabled_with_no_jobs() {
    let out = templates::scheduler_config();
    assert!(out.contains("enabled: false"));
    assert!(out.contains("jobs: []"));
}

#[test]
fn html_templates_expose_expected_placeholders() {
    assert!(templates::page_template().contains("{{ content }}"));
    assert!(templates::page_template().contains("<title>{{ title }}</title>"));

    let post = templates::blog_post_template();
    assert!(post.contains("<h1>{{ title }}</h1>"));
    assert!(post.contains("<time>{{ date }}</time>"));

    let blog_list = templates::blog_list_template();
    assert!(blog_list.contains("{% for post in posts %}"));
    assert!(blog_list.contains("{{ post.url }}"));

    let page_list = templates::page_list_template();
    assert!(page_list.contains("{% for page in pages %}"));
    assert!(page_list.contains("{{ page.url }}"));
}

#[test]
fn welcome_blog_post_has_front_matter_and_project_name() {
    let out = templates::welcome_blog_post("Acme");
    assert!(out.starts_with("---"));
    assert!(out.contains("title: Welcome to Acme"));
    assert!(out.contains("date: 2024-01-01"));
}

#[test]
fn policy_documents_interpolate_project_name() {
    let privacy = templates::privacy_policy("Acme");
    assert!(privacy.contains("title: Privacy Policy"));
    assert!(privacy.contains("placeholder privacy policy for Acme"));

    let cookie = templates::cookie_policy("Acme");
    assert!(cookie.contains("title: Cookie Policy"));
    assert!(cookie.contains("placeholder cookie policy for Acme"));
}
