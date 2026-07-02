//! Tests for the `plugins` command group over the compiled extension registry.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::plugins::{capabilities, config, list, show, validate};
use systemprompt_extension::ExtensionRegistry;

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn first_extension_id() -> String {
    let registry = ExtensionRegistry::discover().unwrap();
    registry
        .extensions()
        .first()
        .map(|e| e.id().to_owned())
        .expect("compiled registry must not be empty")
}

#[test]
fn list_reports_compiled_extensions() {
    let out = list::execute(
        &list::ListArgs {
            filter: None,
            capability: None,
            r#type: "all".to_owned(),
        },
        &cfg(),
    );
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert!(!json["items"].as_array().unwrap().is_empty());
}

#[test]
fn list_applies_filters() {
    let id = first_extension_id();
    let out = list::execute(
        &list::ListArgs {
            filter: Some(id.clone()),
            capability: None,
            r#type: "compiled".to_owned(),
        },
        &cfg(),
    );
    let json = serde_json::to_value(out.artifact()).unwrap();
    let items = json["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .all(|i| i["id"].as_str().unwrap().contains(&id))
    );

    let none = list::execute(
        &list::ListArgs {
            filter: Some("definitely-no-such-extension".to_owned()),
            capability: None,
            r#type: "all".to_owned(),
        },
        &cfg(),
    );
    let json = serde_json::to_value(none.artifact()).unwrap();
    assert!(json["items"].as_array().unwrap().is_empty());

    list::execute(
        &list::ListArgs {
            filter: None,
            capability: Some("jobs".to_owned()),
            r#type: "all".to_owned(),
        },
        &cfg(),
    );
}

#[test]
fn show_renders_known_extension_and_rejects_unknown() {
    let id = first_extension_id();
    let out = show::execute(&show::ShowArgs { id: id.clone() }, &cfg()).unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert_eq!(json["title"], format!("Extension: {id}"));

    let err = show::execute(
        &show::ShowArgs {
            id: "no-such-extension".to_owned(),
        },
        &cfg(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn config_lists_all_and_shows_single() {
    let all = config::execute(&config::ConfigArgs { id: None }, &cfg()).unwrap();
    let json = serde_json::to_value(all.artifact()).unwrap();
    assert!(json.is_object());

    let id = first_extension_id();
    config::execute(&config::ConfigArgs { id: Some(id) }, &cfg()).unwrap();

    let err = config::execute(
        &config::ConfigArgs {
            id: Some("no-such-extension".to_owned()),
        },
        &cfg(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn validate_passes_on_compiled_registry() {
    let (out, ok) = validate::execute(&validate::ValidateArgs { verbose: false }, &cfg());
    assert!(ok);
    serde_json::to_value(out.artifact()).unwrap();

    let (_, ok_verbose) = validate::execute(&validate::ValidateArgs { verbose: true }, &cfg());
    assert!(ok_verbose);
}

#[test]
fn capability_listings_execute_with_and_without_filters() {
    let id = first_extension_id();

    capabilities::jobs::execute(
        &capabilities::jobs::JobsArgs {
            extension: None,
            enabled: false,
        },
        &cfg(),
    );
    capabilities::jobs::execute(
        &capabilities::jobs::JobsArgs {
            extension: Some(id.clone()),
            enabled: true,
        },
        &cfg(),
    );
    capabilities::roles::execute(
        &capabilities::roles::RolesArgs {
            extension: Some(id.clone()),
        },
        &cfg(),
    );
    capabilities::roles::execute(&capabilities::roles::RolesArgs { extension: None }, &cfg());
    capabilities::templates::execute(
        &capabilities::templates::TemplatesArgs { extension: None },
        &cfg(),
    );
    capabilities::schemas::execute(
        &capabilities::schemas::SchemasArgs { extension: None },
        &cfg(),
    );
    capabilities::llm_providers::execute(
        &capabilities::llm_providers::LlmProvidersArgs { extension: None },
        &cfg(),
    );
    capabilities::tools::execute(
        &capabilities::tools::ToolsArgs {
            extension: Some(id),
        },
        &cfg(),
    );
}
