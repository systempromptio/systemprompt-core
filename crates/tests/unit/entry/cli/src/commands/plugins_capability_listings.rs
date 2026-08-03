//! Tests for the per-capability listing commands under `plugins capabilities`.
//!
//! Each submodule projects the compiled extension registry into its own table;
//! none of the six were called by a test.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::plugins::capabilities::{
    jobs, llm_providers, roles, schemas, templates, tools,
};
use systemprompt_extension::ExtensionRegistry;

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn artifact(out: &systemprompt_cli::shared::CommandOutput) -> serde_json::Value {
    serde_json::to_value(out.artifact()).unwrap()
}

fn row_count(out: &systemprompt_cli::shared::CommandOutput) -> usize {
    artifact(out)["items"]
        .as_array()
        .map_or(0, std::vec::Vec::len)
}

fn first_extension_id() -> String {
    ExtensionRegistry::discover()
        .unwrap()
        .extensions()
        .first()
        .map(|e| e.id().to_owned())
        .expect("compiled registry must not be empty")
}

#[test]
fn every_capability_listing_renders_over_the_compiled_registry() {
    let cfg = cfg();

    let outputs = [
        jobs::execute(
            &jobs::JobsArgs {
                extension: None,
                enabled: false,
            },
            &cfg,
        ),
        roles::execute(&roles::RolesArgs { extension: None }, &cfg),
        schemas::execute(&schemas::SchemasArgs { extension: None }, &cfg),
        templates::execute(&templates::TemplatesArgs { extension: None }, &cfg),
        tools::execute(&tools::ToolsArgs { extension: None }, &cfg),
        llm_providers::execute(&llm_providers::LlmProvidersArgs { extension: None }, &cfg),
    ];

    for out in &outputs {
        assert!(
            artifact(out).get("items").is_some(),
            "each listing renders a table"
        );
    }

    assert!(
        outputs.iter().any(|out| row_count(out) > 0),
        "the compiled registry must contribute at least one capability row"
    );
}

#[test]
fn an_unknown_extension_filter_yields_no_rows() {
    let cfg = cfg();
    let absent = Some("cov_no_such_extension".to_owned());

    assert_eq!(
        row_count(&jobs::execute(
            &jobs::JobsArgs {
                extension: absent.clone(),
                enabled: false,
            },
            &cfg
        )),
        0
    );
    assert_eq!(
        row_count(&roles::execute(
            &roles::RolesArgs {
                extension: absent.clone()
            },
            &cfg
        )),
        0
    );
    assert_eq!(
        row_count(&schemas::execute(
            &schemas::SchemasArgs {
                extension: absent.clone()
            },
            &cfg
        )),
        0
    );
    assert_eq!(
        row_count(&templates::execute(
            &templates::TemplatesArgs {
                extension: absent.clone()
            },
            &cfg
        )),
        0
    );
    assert_eq!(
        row_count(&tools::execute(
            &tools::ToolsArgs {
                extension: absent.clone()
            },
            &cfg
        )),
        0
    );
    assert_eq!(
        row_count(&llm_providers::execute(
            &llm_providers::LlmProvidersArgs { extension: absent },
            &cfg
        )),
        0
    );
}

#[test]
fn filtering_by_a_real_extension_never_exceeds_the_unfiltered_listing() {
    let cfg = cfg();
    let id = first_extension_id();

    let all = schemas::execute(&schemas::SchemasArgs { extension: None }, &cfg);
    let filtered = schemas::execute(
        &schemas::SchemasArgs {
            extension: Some(id),
        },
        &cfg,
    );

    assert!(row_count(&filtered) <= row_count(&all));
}

#[test]
fn the_enabled_only_job_listing_is_a_subset_of_all_jobs() {
    let cfg = cfg();

    let all = jobs::execute(
        &jobs::JobsArgs {
            extension: None,
            enabled: false,
        },
        &cfg,
    );
    let enabled = jobs::execute(
        &jobs::JobsArgs {
            extension: None,
            enabled: true,
        },
        &cfg,
    );

    assert!(row_count(&enabled) <= row_count(&all));
}
