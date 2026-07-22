//! Tests for `web content-types create` prompt and resolver helpers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::web::content_types::create::{
    prompt_category_id, prompt_name, prompt_path, prompt_sitemap_config, prompt_source_id,
    resolve_description, resolve_sitemap,
};
use systemprompt_cli::{CliConfig, ScriptedPrompter};
use systemprompt_models::content_config::ContentConfigRaw;

fn interactive() -> CliConfig {
    CliConfig::new()
        .with_interactive(true)
        .with_assume_terminal(true)
}

fn non_interactive() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn config_with_categories(names: &[&str]) -> ContentConfigRaw {
    let yaml = if names.is_empty() {
        "categories: {}\ncontent_sources: {}\n".to_owned()
    } else {
        let mut y = String::from("categories:\n");
        for n in names {
            y.push_str(&format!("  {n}:\n    name: {n}\n    description: d\n"));
        }
        y.push_str("content_sources: {}\n");
        y
    };
    serde_yaml::from_str(&yaml).unwrap()
}

#[test]
fn prompt_name_reprompts_until_valid() {
    let prompter = ScriptedPrompter::new(["X", "a", "has space", "valid-name"]);
    assert_eq!(prompt_name(&prompter).unwrap(), "valid-name");
}

#[test]
fn prompt_path_and_source_default_to_name() {
    let prompter = ScriptedPrompter::new(["", ""]);
    assert_eq!(prompt_path(&prompter, "blog").unwrap(), "content/blog");
    assert_eq!(prompt_source_id(&prompter, "blog").unwrap(), "blog");
}

#[test]
fn prompt_category_id_defaults_without_categories_and_selects_sorted() {
    let prompter = ScriptedPrompter::new([""]);
    let empty = config_with_categories(&[]);
    assert_eq!(prompt_category_id(&prompter, &empty).unwrap(), "blog");

    let cfg = config_with_categories(&["zeta", "alpha"]);
    let prompter = ScriptedPrompter::new(["1"]);
    assert_eq!(prompt_category_id(&prompter, &cfg).unwrap(), "zeta");
}

#[test]
fn prompt_sitemap_config_declined_returns_none() {
    let prompter = ScriptedPrompter::new(["n"]);
    assert!(prompt_sitemap_config(&prompter).unwrap().is_none());
}

#[test]
fn prompt_sitemap_config_reprompts_bad_priority_then_builds() {
    let prompter = ScriptedPrompter::new(["y", "/blog/{slug}", "9.5", "abc", "0.8", "daily"]);
    let sitemap = prompt_sitemap_config(&prompter).unwrap().unwrap();
    assert!(sitemap.enabled);
    assert_eq!(sitemap.url_pattern, "/blog/{slug}");
    assert_eq!(sitemap.priority, 0.8);
    assert_eq!(sitemap.changefreq, "daily");
    assert_eq!(sitemap.fetch_from, "database");
}

#[test]
fn resolve_description_prefers_flag_then_prompt_then_empty() {
    let prompter = ScriptedPrompter::new(["from prompt"]);
    assert_eq!(
        resolve_description(Some("flag".to_owned()), &prompter, &interactive()),
        "flag"
    );
    assert_eq!(
        resolve_description(None, &prompter, &interactive()),
        "from prompt"
    );
    let silent = ScriptedPrompter::new(Vec::<String>::new());
    assert_eq!(resolve_description(None, &silent, &non_interactive()), "");
}

#[test]
fn resolve_sitemap_uses_flags_or_prompts_or_nothing() {
    let silent = ScriptedPrompter::new(Vec::<String>::new());
    let from_flags = resolve_sitemap(
        Some("/x/{slug}".to_owned()),
        0.3,
        "monthly",
        &silent,
        &non_interactive(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(from_flags.url_pattern, "/x/{slug}");
    assert_eq!(from_flags.priority, 0.3);

    assert!(
        resolve_sitemap(None, 0.5, "weekly", &silent, &non_interactive())
            .unwrap()
            .is_none()
    );

    let prompter = ScriptedPrompter::new(["n"]);
    assert!(
        resolve_sitemap(None, 0.5, "weekly", &prompter, &interactive())
            .unwrap()
            .is_none()
    );
}
