//! Tests for the input guards on the local tenant-creation flows.
//!
//! Both flows validate their prompted input before touching the filesystem or
//! a database, so the rejection paths are reachable while the provisioning
//! bodies (which write into the discovered project root) are not.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::tenant::create::{create_external_tenant, create_local_tenant};

#[tokio::test]
async fn an_empty_tenant_name_is_refused_by_the_external_flow() {
    let prompter = ScriptedPrompter::new([""]);

    let err = create_external_tenant(&prompter).await.unwrap_err();
    assert!(err.to_string().contains("Tenant name cannot be empty"));
}

#[tokio::test]
async fn an_empty_database_url_is_refused_by_the_external_flow() {
    let prompter = ScriptedPrompter::new(["covtenant", ""]);

    let err = create_external_tenant(&prompter).await.unwrap_err();
    assert!(err.to_string().contains("Database URL cannot be empty"));
}

#[tokio::test]
async fn an_unreachable_database_is_refused_before_a_profile_is_written() {
    let prompter = ScriptedPrompter::new([
        "covtenant",
        "postgres://nobody:nothing@127.0.0.1:1/absent",
    ]);

    let err = create_external_tenant(&prompter).await.unwrap_err();
    assert!(
        err.to_string().contains("Could not connect to database"),
        "{err}"
    );
}

#[tokio::test]
async fn an_empty_tenant_name_is_refused_by_the_docker_flow() {
    let prompter = ScriptedPrompter::new([""]);

    let err = create_local_tenant(&prompter).await.unwrap_err();
    assert!(err.to_string().contains("Tenant name cannot be empty"));
}

#[tokio::test]
async fn an_exhausted_prompter_is_surfaced_rather_than_defaulted() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());

    let err = create_external_tenant(&prompter).await.unwrap_err();
    assert!(err.to_string().contains("Scripted prompter exhausted"), "{err}");
}

// Why: the port is parsed before any Docker call, so a non-numeric answer must
// be refused here rather than reaching `TenantContainer` with a bad value.
#[tokio::test]
async fn a_non_numeric_port_is_refused_by_the_docker_flow() {
    let prompter = ScriptedPrompter::new(["covtenant", "not-a-number"]);

    let err = create_local_tenant(&prompter).await.unwrap_err();
    assert!(
        format!("{err:#}").contains("PostgreSQL port must be a number"),
        "the refusal must name the port as the problem, got: {err:#}"
    );
}
