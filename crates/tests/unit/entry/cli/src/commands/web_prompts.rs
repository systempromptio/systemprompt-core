//! Tests for the `web content-types` and `web templates` interactive prompts,
//! driven through `ScriptedPrompter` without touching the filesystem.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::web::content_types::create::{
    prompt_category_id, prompt_description, prompt_name, prompt_path, prompt_sitemap_config,
    prompt_source_id,
};
use systemprompt_cli::web::content_types::selection::prompt_content_type_selection;
use systemprompt_cli::web::templates::create::{
    prompt_content_types, prompt_name as prompt_template_name,
};
use systemprompt_cli::web::templates::selection::prompt_template_selection;
use systemprompt_cli::web::types::{TemplateEntry, TemplatesConfig};
use systemprompt_identifiers::{CategoryId, SourceId};
use systemprompt_models::content_config::{Category, ContentConfigRaw, ContentSourceConfigRaw};

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn content_source(name: &str) -> ContentSourceConfigRaw {
    ContentSourceConfigRaw {
        path: format!("content/{name}"),
        source_id: SourceId::new(name),
        category_id: CategoryId::new("blog"),
        enabled: true,
        description: String::new(),
        allowed_content_types: vec![],
        indexing: None,
        sitemap: None,
        branding: None,
    }
}

fn content_config_with_sources(names: &[&str]) -> ContentConfigRaw {
    let mut config = ContentConfigRaw::default();
    for name in names {
        config
            .content_sources
            .insert((*name).to_owned(), content_source(name));
    }
    config
}

fn content_config_with_categories(names: &[&str]) -> ContentConfigRaw {
    let mut config = ContentConfigRaw::default();
    for name in names {
        config
            .categories
            .insert((*name).to_owned(), Category::default());
    }
    config
}

fn templates_config(names: &[&str]) -> TemplatesConfig {
    let mut templates = HashMap::new();
    for name in names {
        templates.insert(
            (*name).to_owned(),
            TemplateEntry {
                content_types: vec!["article".to_owned()],
            },
        );
    }
    TemplatesConfig { templates }
}

#[test]
fn prompt_name_accepts_valid_lowercase_name() {
    let prompter = scripted(&["blog-posts"]);
    let name = prompt_name(&prompter).expect("valid name accepted");
    assert_eq!(name, "blog-posts");
}

#[test]
fn prompt_name_retries_until_valid() {
    let prompter = scripted(&["a", "BAD_NAME", "good-name"]);
    let name = prompt_name(&prompter).expect("valid name after retries");
    assert_eq!(name, "good-name");
}

#[test]
fn prompt_name_exhausts_when_never_valid() {
    let prompter = scripted(&["a"]);
    let err = prompt_name(&prompter).expect_err("no valid answer");
    assert!(err.to_string().contains("exhausted"));
}

#[test]
fn prompt_path_uses_default_on_empty_answer() {
    let prompter = scripted(&[""]);
    let path = prompt_path(&prompter, "blog").expect("default path");
    assert_eq!(path, "content/blog");
}

#[test]
fn prompt_path_accepts_override() {
    let prompter = scripted(&["custom/path"]);
    let path = prompt_path(&prompter, "blog").expect("override path");
    assert_eq!(path, "custom/path");
}

#[test]
fn prompt_source_id_defaults_to_name() {
    let prompter = scripted(&[""]);
    let source = prompt_source_id(&prompter, "blog").expect("default source id");
    assert_eq!(source, "blog");
}

#[test]
fn prompt_category_id_selects_from_list() {
    let config = content_config_with_categories(&["news", "blog", "docs"]);
    let prompter = scripted(&["0"]);
    let category = prompt_category_id(&prompter, &config).expect("selected category");
    assert_eq!(category, "blog");
}

#[test]
fn prompt_category_id_falls_back_to_input_when_empty() {
    let config = ContentConfigRaw::default();
    let prompter = scripted(&[""]);
    let category = prompt_category_id(&prompter, &config).expect("default category");
    assert_eq!(category, "blog");
}

#[test]
fn prompt_description_returns_input() {
    let prompter = scripted(&["a short description"]);
    let description = prompt_description(&prompter).expect("description");
    assert_eq!(description, "a short description");
}

#[test]
fn prompt_sitemap_config_disabled_returns_none() {
    let prompter = scripted(&["n"]);
    let sitemap = prompt_sitemap_config(&prompter).expect("no error");
    assert!(sitemap.is_none());
}

#[test]
fn prompt_sitemap_config_collects_values() {
    let prompter = scripted(&["y", "/blog/{slug}", "0.8", "daily"]);
    let sitemap = prompt_sitemap_config(&prompter)
        .expect("no error")
        .expect("sitemap present");
    assert_eq!(sitemap.url_pattern, "/blog/{slug}");
    assert!((sitemap.priority - 0.8).abs() < f32::EPSILON);
    assert_eq!(sitemap.changefreq, "daily");
}

#[test]
fn prompt_sitemap_config_retries_invalid_priority() {
    let prompter = scripted(&["y", "/blog/{slug}", "9.0", "notanumber", "0.3", "weekly"]);
    let sitemap = prompt_sitemap_config(&prompter)
        .expect("no error")
        .expect("sitemap present");
    assert!((sitemap.priority - 0.3).abs() < f32::EPSILON);
}

#[test]
fn prompt_content_type_selection_returns_choice() {
    let config = content_config_with_sources(&["news", "blog"]);
    let prompter = scripted(&["1"]);
    let selected =
        prompt_content_type_selection(&prompter, &config, "Select").expect("selection made");
    assert_eq!(selected, "news");
}

#[test]
fn prompt_content_type_selection_errors_when_empty() {
    let config = ContentConfigRaw::default();
    let prompter = scripted(&["0"]);
    let err = prompt_content_type_selection(&prompter, &config, "Select").expect_err("no sources");
    assert!(err.to_string().contains("No content types configured"));
}

#[test]
fn template_prompt_name_retries_until_valid() {
    let prompter = scripted(&["X", "landing-page"]);
    let name = prompt_template_name(&prompter).expect("valid template name");
    assert_eq!(name, "landing-page");
}

#[test]
fn prompt_content_types_splits_comma_list() {
    let prompter = scripted(&["blog, news ,docs"]);
    let types = prompt_content_types(&prompter).expect("content types");
    assert_eq!(types, vec!["blog", "news", "docs"]);
}

#[test]
fn prompt_template_selection_returns_choice() {
    let config = templates_config(&["landing", "about"]);
    let prompter = scripted(&["1"]);
    let selected = prompt_template_selection(&prompter, &config, "Select").expect("selection made");
    assert_eq!(selected, "landing");
}

#[test]
fn prompt_template_selection_errors_when_empty() {
    let config = templates_config(&[]);
    let prompter = scripted(&["0"]);
    let err = prompt_template_selection(&prompter, &config, "Select").expect_err("no templates");
    assert!(err.to_string().contains("No templates configured"));
}
