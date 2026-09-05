//! Harness tests for the interactive tenant flows: edit, the create menu,
//! and external-database tenant creation end to end.

use systemprompt_cli::cloud::tenant::TenantCommands;
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::TenantStore;
use systemprompt_identifiers::TenantId;

use super::{OTHER_TENANT_ID, TENANT_ID, enter, interactive_ctx, json_ctx};
use crate::full_bootstrap::database_url_or_skip;

fn tenant_cmd(command: TenantCommands) -> CloudCommands {
    CloudCommands::Tenant {
        command: Some(command),
    }
}

#[tokio::test]
async fn tenant_edit_requires_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(OTHER_TENANT_ID.to_owned()),
        }),
        &json_ctx(),
    )
    .await
    .expect_err("edit needs interactive");
    assert!(err.to_string().contains("interactive"));
}

#[tokio::test]
async fn tenant_edit_local_renames_and_edits_database() {
    let env = enter().await;
    let ctx = interactive_ctx(["renamed-local", "y", "postgres://u:p@edited:5432/db"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(OTHER_TENANT_ID.to_owned()),
        }),
        &ctx,
    )
    .await
    .expect("edit local tenant");

    let store = TenantStore::load_from_path(&env.root().join(".systemprompt/tenants.json"))
        .expect("reload tenants");
    let tenant = store
        .find_tenant(&TenantId::new(OTHER_TENANT_ID))
        .expect("tenant");
    assert_eq!(tenant.name, "renamed-local");
    assert_eq!(
        tenant.database_url.as_deref(),
        Some("postgres://u:p@edited:5432/db")
    );
}

#[tokio::test]
async fn tenant_edit_local_declines_database_edit() {
    let _env = enter().await;
    let ctx = interactive_ctx(["kept-name", "n"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(OTHER_TENANT_ID.to_owned()),
        }),
        &ctx,
    )
    .await
    .expect("edit declining db change");
}

#[tokio::test]
async fn tenant_edit_cloud_shows_readonly_fields() {
    let _env = enter().await;
    let ctx = interactive_ctx(["Harness Prod Renamed"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(TENANT_ID.to_owned()),
        }),
        &ctx,
    )
    .await
    .expect("edit cloud tenant");
}

#[tokio::test]
async fn tenant_edit_unknown_id_errors() {
    let _env = enter().await;
    let ctx = interactive_ctx(Vec::<String>::new());
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some("nope".to_owned()),
        }),
        &ctx,
    )
    .await
    .expect_err("unknown tenant");
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn tenant_create_requires_interactive() {
    let _env = enter().await;
    let err = cloud::execute(tenant_cmd(TenantCommands::Create), &json_ctx())
        .await
        .expect_err("create needs interactive");
    assert!(err.to_string().contains("interactive"));
}

#[tokio::test]
async fn tenant_create_external_rejects_empty_inputs() {
    let _env = enter().await;

    let ctx = interactive_ctx(["1", "ext-tenant", ""]);
    let err = cloud::execute(tenant_cmd(TenantCommands::Create), &ctx)
        .await
        .expect_err("empty database url");
    assert!(err.to_string().contains("Database URL"));

    let ctx = interactive_ctx(["1", "ext-tenant", "postgres://u:p@127.0.0.1:1/void"]);
    let err = cloud::execute(tenant_cmd(TenantCommands::Create), &ctx)
        .await
        .expect_err("unreachable database");
    assert!(err.to_string().contains("connect"));
}

#[tokio::test]
async fn tenant_create_external_full_flow() {
    let Some(url) = database_url_or_skip() else { return };
    let env = enter().await;
    let profiles = env.root().join(".systemprompt/profiles/ext-prof");
    if profiles.exists() {
        std::fs::remove_dir_all(&profiles).expect("clean ext profile");
    }

    let ctx = interactive_ctx([
        "1",
        "ext-tenant",
        url.as_str(),
        "ext-prof",
        "0",
        "ext-gemini-key",
        "n",
        "n",
    ]);
    cloud::execute(tenant_cmd(TenantCommands::Create), &ctx)
        .await
        .expect("external tenant create");

    let store = TenantStore::load_from_path(&env.root().join(".systemprompt/tenants.json"))
        .expect("reload tenants");
    assert!(store.tenants.iter().any(|t| t.name == "ext-tenant"));
    assert!(profiles.join("profile.yaml").exists());

    std::fs::remove_dir_all(&profiles).expect("clean ext profile");
}

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::tenant::docker::{TenantContainer, container};
use systemprompt_cli::cloud::tenant::{
    TenantDeleteArgs, TenantRotateArgs, choose_tenant_operation,
};
use systemprompt_cloud::{CommandRunner, CommandSpec, DockerCli};

enum Resp {
    Out(i32, &'static str),
}

struct StubRunner {
    responses: Mutex<Vec<Resp>>,
}

impl StubRunner {
    fn docker(responses: Vec<Resp>) -> DockerCli {
        DockerCli::with_runner(Box::new(Self {
            responses: Mutex::new(responses),
        }))
    }

    fn next(&self, spec: &CommandSpec) -> Resp {
        let mut responses = self.responses.lock().expect("stub lock");
        if responses.is_empty() {
            panic!("StubRunner exhausted for {spec:?}");
        }
        responses.remove(0)
    }
}

impl CommandRunner for StubRunner {
    fn output(&self, spec: &CommandSpec) -> io::Result<Output> {
        match self.next(spec) {
            Resp::Out(code, stdout) => Ok(Output {
                status: ExitStatus::from_raw(code << 8),
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
            }),
        }
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        match self.next(spec) {
            Resp::Out(code, _) => Ok(ExitStatus::from_raw(code << 8)),
        }
    }

    fn status_with_stdin(&self, spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        self.status(spec)
    }
}

#[test]
fn each_tenant_gets_its_own_compose_project() {
    let a = TenantContainer::new("acme_01".to_owned(), "pw-a".to_owned(), 5432);
    let b = TenantContainer::new("acme_02".to_owned(), "pw-b".to_owned(), 5433);

    assert_ne!(a.compose_path(), b.compose_path());
    assert_ne!(a.database_url(), b.database_url());
}

#[test]
fn is_project_running_is_scoped_to_the_named_project() {
    let running = StubRunner::docker(vec![Resp::Out(0, "container-id\n")]);
    assert!(container::is_project_running(&running, "acme_01"));

    let absent = StubRunner::docker(vec![Resp::Out(0, "")]);
    assert!(!container::is_project_running(&absent, "acme_01"));
}

#[test]
fn tenant_operation_menu_maps_selections() {
    for (answer, has_tenants) in [
        ("0", true),
        ("1", true),
        ("2", true),
        ("3", true),
        ("2", false),
    ] {
        let prompter = ScriptedPrompter::new([answer]);
        let cmd = choose_tenant_operation(&prompter, has_tenants).expect("menu selection");
        assert!(cmd.is_some());
    }
    let prompter = ScriptedPrompter::new(["4"]);
    assert!(
        choose_tenant_operation(&prompter, true)
            .expect("done")
            .is_none()
    );
}

#[tokio::test]
async fn tenant_menu_non_interactive_requires_subcommand() {
    let _env = enter().await;
    let err = cloud::execute(CloudCommands::Tenant { command: None }, &json_ctx())
        .await
        .expect_err("needs subcommand");
    assert!(err.to_string().contains("subcommand"));
}

#[tokio::test]
async fn tenant_menu_interactive_list_then_done() {
    let _env = enter().await;
    let ctx = interactive_ctx(["1", "2", "4"]);
    cloud::execute(CloudCommands::Tenant { command: None }, &ctx)
        .await
        .expect("tenant menu");
}

#[tokio::test]
async fn tenant_delete_interactive_picker_and_cancel() {
    let _env = enter().await;
    let ctx = interactive_ctx(["1", "n"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Delete(TenantDeleteArgs {
            id: None,
            yes: false,
        })),
        &ctx,
    )
    .await
    .expect("cancelled tenant delete");
}

#[tokio::test]
async fn tenant_delete_interactive_confirm_local() {
    let env = enter().await;
    let ctx = interactive_ctx(["1", "y"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Delete(TenantDeleteArgs {
            id: None,
            yes: false,
        })),
        &ctx,
    )
    .await
    .expect("delete local tenant");
    let store = TenantStore::load_from_path(&env.root().join(".systemprompt/tenants.json"))
        .expect("reload tenants");
    assert!(store.find_tenant(&TenantId::new(OTHER_TENANT_ID)).is_none());
}

#[tokio::test]
async fn tenant_delete_removes_a_managed_container_tenant() {
    let env = enter().await;
    let managed = systemprompt_cloud::StoredTenant::new_local_docker(
        TenantId::new("t-managed"),
        "Managed Local".to_owned(),
        "postgres://u:p@localhost:5432/systemprompt".to_owned(),
        "managed_project".to_owned(),
    );
    let tenants_path = env.root().join(".systemprompt/tenants.json");
    let mut store = TenantStore::load_from_path(&tenants_path).expect("load tenants");
    store.tenants.push(managed);
    store
        .save_to_path(&tenants_path)
        .expect("seed managed tenant");

    cloud::execute(
        tenant_cmd(TenantCommands::Delete(TenantDeleteArgs {
            id: Some("t-managed".to_owned()),
            yes: true,
        })),
        &json_ctx(),
    )
    .await
    .expect("delete managed tenant");
}

#[tokio::test]
async fn tenant_rotate_interactive_confirm_and_cancel() {
    let env = enter().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path(format!(
            "/api/v1/tenants/{TENANT_ID}/rotate-credentials"
        )))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "rotated",
                "message": "ok",
                "internal_database_url": "postgres://int/rotated2",
                "external_database_url": "postgres://ext/rotated2"
            })),
        )
        .mount(env.server())
        .await;

    let cancel = interactive_ctx(["0", "n"]);
    cloud::execute(
        tenant_cmd(TenantCommands::RotateCredentials(TenantRotateArgs {
            id: None,
            yes: false,
        })),
        &cancel,
    )
    .await
    .expect("cancelled rotation");

    let confirm = interactive_ctx(["0", "y"]);
    cloud::execute(
        tenant_cmd(TenantCommands::RotateCredentials(TenantRotateArgs {
            id: None,
            yes: false,
        })),
        &confirm,
    )
    .await
    .expect("confirmed rotation");
}

#[tokio::test]
async fn tenant_show_interactive_picker() {
    let _env = enter().await;
    let ctx = interactive_ctx(["0"]);
    cloud::execute(tenant_cmd(TenantCommands::Show { id: None }), &ctx)
        .await
        .expect("interactive show");
}

#[tokio::test]
async fn tenant_list_interactive_details_then_back() {
    let _env = enter().await;
    let ctx = interactive_ctx(["0", "1", "2"]);
    cloud::execute(tenant_cmd(TenantCommands::List), &ctx)
        .await
        .expect("interactive list");
}
