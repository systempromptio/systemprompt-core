//! Harness tests for the interactive tenant flows: edit, the create menu,
//! and external-database tenant creation end to end.

use systemprompt_cli::cloud::tenant::TenantCommands;
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::TenantStore;
use systemprompt_identifiers::TenantId;

use super::{OTHER_TENANT_ID, TENANT_ID, enter, interactive_ctx, json_ctx};
use crate::full_bootstrap::database_url;

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
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &json_ctx(),
    )
    .await
    .expect_err("create needs interactive");
    assert!(err.to_string().contains("interactive"));
}

#[tokio::test]
async fn tenant_create_cloud_unavailable_without_release_build() {
    let _env = enter().await;
    let ctx = interactive_ctx(["1"]);
    let result = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn tenant_create_external_rejects_empty_inputs() {
    let _env = enter().await;

    let ctx = interactive_ctx(["0", "1", "ext-tenant", ""]);
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
    .await
    .expect_err("empty database url");
    assert!(err.to_string().contains("Database URL"));

    let ctx = interactive_ctx(["0", "1", "ext-tenant", "postgres://u:p@127.0.0.1:1/void"]);
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
    .await
    .expect_err("unreachable database");
    assert!(err.to_string().contains("connect"));
}

#[tokio::test]
async fn tenant_create_external_full_flow() {
    let Some(url) = database_url() else { return };
    let env = enter().await;
    let profiles = env.root().join(".systemprompt/profiles/ext-prof");
    if profiles.exists() {
        std::fs::remove_dir_all(&profiles).expect("clean ext profile");
    }

    let ctx = interactive_ctx([
        "0",
        "1",
        "ext-tenant",
        url.as_str(),
        "ext-prof",
        "0",
        "ext-gemini-key",
        "n",
        "n",
    ]);
    cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
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
use systemprompt_cli::cloud::tenant::docker::SharedContainerConfig;
use systemprompt_cli::cloud::tenant::{
    TenantCancelArgs, TenantDeleteArgs, TenantRotateArgs, choose_tenant_operation,
    handle_orphaned_volume, resolve_container_state,
};
use systemprompt_cloud::{CommandRunner, CommandSpec, DockerCli};

enum Resp {
    Out(i32, &'static str),
    Status(i32),
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
            Resp::Status(code) => Ok(Output {
                status: ExitStatus::from_raw(code << 8),
                stdout: Vec::new(),
                stderr: Vec::new(),
            }),
        }
    }

    fn status(&self, spec: &CommandSpec) -> io::Result<ExitStatus> {
        match self.next(spec) {
            Resp::Out(code, _) | Resp::Status(code) => Ok(ExitStatus::from_raw(code << 8)),
        }
    }

    fn status_with_stdin(&self, spec: &CommandSpec, _stdin: &[u8]) -> io::Result<ExitStatus> {
        self.status(spec)
    }
}

#[test]
fn container_state_reuses_running_container_with_config() {
    let docker = StubRunner::docker(vec![]);
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let config = SharedContainerConfig::new("pw".to_owned(), 5432);
    let (resolved, needs_start) =
        resolve_container_state(&docker, Some(config), true, &prompter).expect("reuse");
    assert!(!needs_start);
    assert_eq!(resolved.admin_password, "pw");
}

#[test]
fn container_state_restarts_stopped_container_with_config() {
    let docker = StubRunner::docker(vec![]);
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let config = SharedContainerConfig::new("pw".to_owned(), 5432);
    let (_, needs_start) =
        resolve_container_state(&docker, Some(config), false, &prompter).expect("restart");
    assert!(needs_start);
}

#[test]
fn container_state_adopts_existing_container_password() {
    let docker = StubRunner::docker(vec![Resp::Out(
        0,
        "POSTGRES_USER=admin\nPOSTGRES_PASSWORD=found-pw\n",
    )]);
    let prompter = ScriptedPrompter::new(["y"]);
    let (config, needs_start) =
        resolve_container_state(&docker, None, true, &prompter).expect("adopt");
    assert!(!needs_start);
    assert_eq!(config.admin_password, "found-pw");
}

#[test]
fn container_state_rejects_existing_container_when_declined() {
    let docker = StubRunner::docker(vec![]);
    let prompter = ScriptedPrompter::new(["n"]);
    let err = resolve_container_state(&docker, None, true, &prompter).expect_err("declined");
    assert!(err.to_string().contains("docker stop"));
}

#[test]
fn container_state_errors_when_password_unavailable() {
    let docker = StubRunner::docker(vec![Resp::Out(0, "POSTGRES_USER=admin\n")]);
    let prompter = ScriptedPrompter::new(["y"]);
    let err = resolve_container_state(&docker, None, true, &prompter).expect_err("no password");
    assert!(err.to_string().contains("password"));
}

#[test]
fn container_state_creates_fresh_container_without_volume() {
    let docker = StubRunner::docker(vec![Resp::Out(0, "")]);
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let (config, needs_start) =
        resolve_container_state(&docker, None, false, &prompter).expect("fresh");
    assert!(needs_start);
    assert!(!config.admin_password.is_empty());
}

#[test]
fn orphaned_volume_reset_removes_volume() {
    let docker = StubRunner::docker(vec![Resp::Out(0, "vol-id\n"), Resp::Status(0)]);
    let prompter = ScriptedPrompter::new(["y"]);
    handle_orphaned_volume(&docker, &prompter).expect("volume reset");
}

#[test]
fn orphaned_volume_kept_blocks_creation() {
    let docker = StubRunner::docker(vec![Resp::Out(0, "vol-id\n")]);
    let prompter = ScriptedPrompter::new(["n"]);
    let err = handle_orphaned_volume(&docker, &prompter).expect_err("kept volume blocks");
    assert!(err.to_string().contains("docker volume rm"));
}

#[test]
fn orphaned_volume_remove_failure_bubbles() {
    let docker = StubRunner::docker(vec![Resp::Out(0, "vol-id\n"), Resp::Status(1)]);
    let prompter = ScriptedPrompter::new(["y"]);
    let err = handle_orphaned_volume(&docker, &prompter).expect_err("rm failed");
    assert!(err.to_string().contains("Failed to remove volume"));
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
async fn tenant_delete_shared_container_without_config_warns() {
    let env = enter().await;
    let shared = systemprompt_cloud::StoredTenant::new_local_shared(
        TenantId::new("t-shared"),
        "Shared Local".to_owned(),
        "postgres://u:p@localhost:5432/shared_db".to_owned(),
        "shared_db".to_owned(),
    );
    let tenants_path = env.root().join(".systemprompt/tenants.json");
    let mut store = TenantStore::load_from_path(&tenants_path).expect("load tenants");
    store.tenants.push(shared);
    store
        .save_to_path(&tenants_path)
        .expect("seed shared tenant");

    cloud::execute(
        tenant_cmd(TenantCommands::Delete(TenantDeleteArgs {
            id: Some("t-shared".to_owned()),
            yes: true,
        })),
        &json_ctx(),
    )
    .await
    .expect("delete shared tenant without config");
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
async fn tenant_cancel_aborts_on_name_mismatch() {
    let _env = enter().await;
    let ctx = interactive_ctx(["0", "wrong-name"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Cancel(TenantCancelArgs { id: None })),
        &ctx,
    )
    .await
    .expect("aborted cancellation");
}

#[tokio::test]
async fn tenant_cancel_confirmed_calls_api() {
    let env = enter().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path(format!(
            "/api/v1/tenants/{TENANT_ID}/subscription/cancel"
        )))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::Value::Null))
        .mount(env.server())
        .await;

    let ctx = interactive_ctx(["Harness Prod"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Cancel(TenantCancelArgs {
            id: Some(TENANT_ID.to_owned()),
        })),
        &ctx,
    )
    .await
    .expect("confirmed cancellation");
}

#[tokio::test]
async fn tenant_list_interactive_details_then_back() {
    let _env = enter().await;
    let ctx = interactive_ctx(["0", "1", "2"]);
    cloud::execute(tenant_cmd(TenantCommands::List), &ctx)
        .await
        .expect("interactive list");
}
