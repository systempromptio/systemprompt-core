//! Harness tests driving the `cloud profile` subcommands (show, list,
//! delete, edit, create) through `cloud::execute` with scripted prompts.

use std::path::{Path, PathBuf};
use systemprompt_identifiers::TenantId;

use serde_json::json;
use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::profile::{
    CreateArgs, DeleteArgs, EditArgs, ProfileCommands, ShowFilter, TenantTypeArg,
    create_profile_for_tenant,
};
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::tenants::{NewCloudTenantParams, StoredTenant, TenantStore};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{Env, TENANT_ID, enter, interactive_ctx, json_ctx, table_ctx};

fn profile_cmd(command: ProfileCommands) -> CloudCommands {
    CloudCommands::Profile {
        command: Some(command),
    }
}

fn scaffold_scratch_profile(env: &Env, name: &str) -> PathBuf {
    let dir = env.root().join(".systemprompt/profiles").join(name);
    std::fs::create_dir_all(&dir).expect("scratch profile dir");
    std::fs::copy(
        env.root().join(".systemprompt/profiles/local/profile.yaml"),
        dir.join("profile.yaml"),
    )
    .expect("copy profile.yaml");
    std::fs::write(
        dir.join("secrets.json"),
        r#"{"gemini":"g0","database_url":"postgres://u:p@localhost:5432/db"}"#,
    )
    .expect("write scratch secrets");
    dir
}

fn remove_scratch_profile(env: &Env, name: &str) {
    let dir = env.root().join(".systemprompt/profiles").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove scratch profile");
    }
}

#[tokio::test]
async fn show_renders_every_filter() {
    let env = enter().await;
    let _ = env;
    for filter in [
        ShowFilter::All,
        ShowFilter::Agents,
        ShowFilter::Mcp,
        ShowFilter::Skills,
        ShowFilter::Ai,
        ShowFilter::Web,
        ShowFilter::Content,
        ShowFilter::Env,
        ShowFilter::Settings,
    ] {
        cloud::execute(
            profile_cmd(ProfileCommands::Show {
                name: Some("local".to_owned()),
                filter,
                json: false,
                yaml: false,
            }),
            &table_ctx(),
        )
        .await
        .expect("profile show filter");
    }
}

#[tokio::test]
async fn show_renders_json_and_yaml() {
    let _env = enter().await;
    for (json, yaml) in [(true, false), (false, true)] {
        cloud::execute(
            profile_cmd(ProfileCommands::Show {
                name: Some("local".to_owned()),
                filter: ShowFilter::All,
                json,
                yaml,
            }),
            &json_ctx(),
        )
        .await
        .expect("profile show json/yaml");
    }
}

#[tokio::test]
async fn list_renders_json_and_table() {
    let _env = enter().await;
    cloud::execute(profile_cmd(ProfileCommands::List), &json_ctx())
        .await
        .expect("profile list json");
    cloud::execute(profile_cmd(ProfileCommands::List), &table_ctx())
        .await
        .expect("profile list table");
}

#[tokio::test]
async fn list_interactive_picker_shows_then_backs_out() {
    let _env = enter().await;
    let ctx = interactive_ctx(["0", "1"]);
    cloud::execute(profile_cmd(ProfileCommands::List), &ctx)
        .await
        .expect("profile list picker");
}

#[tokio::test]
async fn interactive_operation_menu_lists_then_done() {
    let _env = enter().await;
    let ctx = interactive_ctx(["0", "1", "3"]);
    cloud::execute(CloudCommands::Profile { command: None }, &ctx)
        .await
        .expect("profile operation menu");
}

#[tokio::test]
async fn profile_requires_subcommand_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(CloudCommands::Profile { command: None }, &json_ctx())
        .await
        .expect_err("needs subcommand");
    assert!(err.to_string().contains("subcommand"));
}

#[tokio::test]
async fn delete_flows() {
    let env = enter().await;

    let err = cloud::execute(
        profile_cmd(ProfileCommands::Delete(DeleteArgs {
            name: "missing-prof".to_owned(),
            yes: true,
        })),
        &json_ctx(),
    )
    .await
    .expect_err("missing profile");
    assert!(err.to_string().contains("does not exist"));

    let bare = env.root().join(".systemprompt/profiles/bare-dir");
    std::fs::create_dir_all(&bare).expect("bare dir");
    let err = cloud::execute(
        profile_cmd(ProfileCommands::Delete(DeleteArgs {
            name: "bare-dir".to_owned(),
            yes: true,
        })),
        &json_ctx(),
    )
    .await
    .expect_err("not a profile");
    assert!(err.to_string().contains("profile.yaml"));
    std::fs::remove_dir_all(&bare).expect("remove bare dir");

    scaffold_scratch_profile(&env, "del-me");
    let err = cloud::execute(
        profile_cmd(ProfileCommands::Delete(DeleteArgs {
            name: "del-me".to_owned(),
            yes: false,
        })),
        &json_ctx(),
    )
    .await
    .expect_err("needs --yes");
    assert!(err.to_string().contains("--yes"));

    let cancel_ctx = interactive_ctx(["n"]);
    cloud::execute(
        profile_cmd(ProfileCommands::Delete(DeleteArgs {
            name: "del-me".to_owned(),
            yes: false,
        })),
        &cancel_ctx,
    )
    .await
    .expect("cancelled delete");
    assert!(env.root().join(".systemprompt/profiles/del-me").exists());

    cloud::execute(
        profile_cmd(ProfileCommands::Delete(DeleteArgs {
            name: "del-me".to_owned(),
            yes: true,
        })),
        &table_ctx(),
    )
    .await
    .expect("delete profile");
    assert!(!env.root().join(".systemprompt/profiles/del-me").exists());
}

fn edit_args(name: &str) -> EditArgs {
    EditArgs {
        name: Some(name.to_owned()),
        set_anthropic_key: None,
        set_openai_key: None,
        set_gemini_key: None,
        set_github_token: None,
        set_database_url: None,
        set_external_url: None,
        set_host: None,
        set_port: None,
    }
}

#[tokio::test]
async fn edit_applies_flag_updates() {
    let env = enter().await;
    let dir = scaffold_scratch_profile(&env, "edit-flags");

    let mut args = edit_args("edit-flags");
    args.set_anthropic_key = Some("ak".to_owned());
    args.set_openai_key = Some("ok".to_owned());
    args.set_gemini_key = Some("gk".to_owned());
    args.set_github_token = Some("ght".to_owned());
    args.set_database_url = Some("postgres://u:p@db:5432/x".to_owned());
    args.set_external_url = Some("https://edited.example.com".to_owned());
    args.set_host = Some("0.0.0.0".to_owned());
    args.set_port = Some(9911);

    cloud::execute(profile_cmd(ProfileCommands::Edit(args)), &json_ctx())
        .await
        .expect("edit with flags");

    let secrets: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("secrets.json")).unwrap()).unwrap();
    assert_eq!(secrets["anthropic"], json!("ak"));
    assert_eq!(secrets["openai"], json!("ok"));
    assert_eq!(secrets["gemini"], json!("gk"));
    assert_eq!(secrets["github"], json!("ght"));
    assert_eq!(secrets["database_url"], json!("postgres://u:p@db:5432/x"));

    let profile = std::fs::read_to_string(dir.join("profile.yaml")).unwrap();
    assert!(profile.contains("edited.example.com"));
    assert!(profile.contains("9911"));

    remove_scratch_profile(&env, "edit-flags");
}

#[tokio::test]
async fn edit_without_flags_errors_non_interactive() {
    let env = enter().await;
    scaffold_scratch_profile(&env, "edit-ni");
    let err = cloud::execute(
        profile_cmd(ProfileCommands::Edit(edit_args("edit-ni"))),
        &json_ctx(),
    )
    .await
    .expect_err("requires flags");
    assert!(err.to_string().contains("--set-"));
    remove_scratch_profile(&env, "edit-ni");
}

#[tokio::test]
async fn edit_interactive_api_keys_menu() {
    let env = enter().await;
    let dir = scaffold_scratch_profile(&env, "edit-menu");

    let ctx = interactive_ctx([
        "3",
        "0",
        "new-gemini",
        "1",
        "new-anthropic",
        "2",
        "",
        "3",
        "postgres://u:p@edited:5432/db",
        "4",
        "4",
    ]);
    cloud::execute(
        profile_cmd(ProfileCommands::Edit(edit_args("edit-menu"))),
        &ctx,
    )
    .await
    .expect("interactive edit");

    let secrets: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("secrets.json")).unwrap()).unwrap();
    assert_eq!(secrets["gemini"], json!("new-gemini"));
    assert_eq!(secrets["anthropic"], json!("new-anthropic"));
    assert_eq!(
        secrets["database_url"],
        json!("postgres://u:p@edited:5432/db")
    );

    remove_scratch_profile(&env, "edit-menu");
}

fn create_args(name: &str, tenant: Option<&str>, tenant_type: TenantTypeArg) -> CreateArgs {
    CreateArgs {
        name: name.to_owned(),
        tenant: tenant.map(str::to_owned),
        tenant_type,
        anthropic_key: None,
        openai_key: None,
        gemini_key: Some("test-gemini-key".to_owned()),
        github_token: None,
    }
}

#[tokio::test]
async fn create_non_interactive_local_tenant() {
    let env = enter().await;
    remove_scratch_profile(&env, "created-local");

    cloud::execute(
        profile_cmd(ProfileCommands::Create(create_args(
            "created-local",
            Some(super::OTHER_TENANT_ID),
            TenantTypeArg::Local,
        ))),
        &json_ctx(),
    )
    .await
    .expect("create local profile");

    let dir = env.root().join(".systemprompt/profiles/created-local");
    assert!(dir.join("profile.yaml").exists());
    assert!(dir.join("secrets.json").exists());
    assert!(dir.join("docker").exists());

    remove_scratch_profile(&env, "created-local");
}

#[tokio::test]
async fn create_rejects_duplicate_and_bad_tenants() {
    let env = enter().await;

    let err = cloud::execute(
        profile_cmd(ProfileCommands::Create(create_args(
            "local",
            Some(super::OTHER_TENANT_ID),
            TenantTypeArg::Local,
        ))),
        &json_ctx(),
    )
    .await
    .expect_err("duplicate profile");
    assert!(err.to_string().contains("already exists"));

    let err = cloud::execute(
        profile_cmd(ProfileCommands::Create(create_args(
            "np1",
            None,
            TenantTypeArg::Local,
        ))),
        &json_ctx(),
    )
    .await
    .expect_err("tenant-id required");
    assert!(err.to_string().contains("--tenant-id"));

    let err = cloud::execute(
        profile_cmd(ProfileCommands::Create(create_args(
            "np2",
            Some("no-such-tenant"),
            TenantTypeArg::Local,
        ))),
        &json_ctx(),
    )
    .await
    .expect_err("unknown tenant");
    assert!(err.to_string().contains("not found"));

    let err = cloud::execute(
        profile_cmd(ProfileCommands::Create(create_args(
            "np3",
            Some(super::OTHER_TENANT_ID),
            TenantTypeArg::Cloud,
        ))),
        &json_ctx(),
    )
    .await
    .expect_err("type mismatch");
    assert!(err.to_string().contains("tenant-type"));

    let mut no_keys = create_args("np4", Some(super::OTHER_TENANT_ID), TenantTypeArg::Local);
    no_keys.gemini_key = None;
    let err = cloud::execute(profile_cmd(ProfileCommands::Create(no_keys)), &json_ctx())
        .await
        .expect_err("api key required");
    assert!(err.to_string().contains("API key"));

    let _ = env;
}

async fn mount_masked_refresh(server: &MockServer) {
    let secrets_path = format!("/api/v1/tenants/{TENANT_ID}/secrets-doc");
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/status")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "status": "active",
                "secrets_url": format!("{}{}", server.uri(), secrets_path),
            }
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(secrets_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "database_url": "postgres://u:realpass@ext.example.com:5432/db",
            "internal_database_url": "postgres://u:realpass@int.example.com:5432/db",
            "app_url": "https://harness.example.com"
        })))
        .mount(server)
        .await;
}

fn seed_masked_cloud_tenant(env: &Env) -> PathBuf {
    let tenant = StoredTenant::new_cloud(NewCloudTenantParams {
        id: TenantId::new(TENANT_ID),
        name: "Masked Prod".to_owned(),
        app_id: Some("app-masked".to_owned()),
        hostname: Some("masked.example.com".to_owned()),
        region: Some("iad".to_owned()),
        database_url: Some("postgres://u:***@ext.example.com:5432/db".to_owned()),
        internal_database_url: "postgres://u:***@int.example.com:5432/db".to_owned(),
        external_db_access: true,
    });
    let tenants_path = env.root().join(".systemprompt/tenants.json");
    TenantStore::new(vec![tenant])
        .save_to_path(&tenants_path)
        .expect("seed masked tenant");
    tenants_path
}

#[tokio::test]
async fn create_cloud_tenant_refreshes_masked_credentials() {
    let env = enter().await;
    remove_scratch_profile(&env, "created-cloud");
    let tenants_path = seed_masked_cloud_tenant(&env);
    mount_masked_refresh(env.server()).await;

    cloud::execute(
        profile_cmd(ProfileCommands::Create(create_args(
            "created-cloud",
            Some(TENANT_ID),
            TenantTypeArg::Cloud,
        ))),
        &json_ctx(),
    )
    .await
    .expect("create cloud profile with refresh");

    let store = TenantStore::load_from_path(&tenants_path).expect("reload tenants");
    let tenant = store
        .find_tenant(&TenantId::new(TENANT_ID))
        .expect("tenant present");
    assert_eq!(
        tenant.internal_database_url.as_deref(),
        Some("postgres://u:realpass@int.example.com:5432/db")
    );
    assert_eq!(
        tenant.database_url.as_deref(),
        Some("postgres://u:realpass@ext.example.com:5432/db")
    );

    let dir = env.root().join(".systemprompt/profiles/created-cloud");
    let profile = std::fs::read_to_string(dir.join("profile.yaml")).unwrap();
    assert!(profile.contains("masked.example.com"));

    remove_scratch_profile(&env, "created-cloud");
}

#[tokio::test]
async fn create_cloud_tenant_tolerates_refresh_failure() {
    let env = enter().await;
    remove_scratch_profile(&env, "created-cloud-fail");
    seed_masked_cloud_tenant(&env);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/status")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    cloud::execute(
        profile_cmd(ProfileCommands::Create(create_args(
            "created-cloud-fail",
            Some(TENANT_ID),
            TenantTypeArg::Cloud,
        ))),
        &json_ctx(),
    )
    .await
    .expect("create proceeds despite refresh failure");

    remove_scratch_profile(&env, "created-cloud-fail");
}

#[tokio::test]
async fn create_interactive_selects_tenant_and_keys() {
    let env = enter().await;
    remove_scratch_profile(&env, "created-inter");

    let ctx = interactive_ctx(["0", "0", "1", "interactive-anthropic-key"]);
    let mut args = create_args("created-inter", None, TenantTypeArg::Local);
    args.gemini_key = None;
    cloud::execute(profile_cmd(ProfileCommands::Create(args)), &ctx)
        .await
        .expect("interactive create");

    let dir = env.root().join(".systemprompt/profiles/created-inter");
    let secrets = std::fs::read_to_string(dir.join("secrets.json")).unwrap();
    assert!(secrets.contains("interactive-anthropic-key"));

    remove_scratch_profile(&env, "created-inter");
}

#[tokio::test]
async fn create_profile_for_tenant_handles_name_collision_and_issuer() {
    let env = enter().await;
    remove_scratch_profile(&env, "collide-renamed");
    scaffold_scratch_profile(&env, "collide");

    let store = TenantStore::load_from_path(&env.root().join(".systemprompt/tenants.json"))
        .expect("load tenants");
    let tenant = store
        .find_tenant(&TenantId::new(TENANT_ID))
        .expect("cloud tenant")
        .clone();

    let prompter = ScriptedPrompter::new(["collide-renamed"]);
    let api_keys =
        systemprompt_cli::cloud::profile::ApiKeys::from_options(Some("g".to_owned()), None, None)
            .expect("api keys");

    let created = create_profile_for_tenant(
        &prompter,
        &tenant,
        &api_keys,
        "collide",
        Some("https://control.example.com/"),
    )
    .expect("create profile for tenant");
    assert_eq!(created.name, "collide-renamed");

    let profile_yaml = std::fs::read_to_string(
        env.root()
            .join(".systemprompt/profiles/collide-renamed/profile.yaml"),
    )
    .unwrap();
    assert!(profile_yaml.contains("https://control.example.com"));
    assert!(profile_yaml.contains("jwks.json"));

    remove_scratch_profile(&env, "collide");
    remove_scratch_profile(&env, "collide-renamed");
}

#[tokio::test]
async fn redact_database_url_variants() {
    let _env = Path::new(".");
    use systemprompt_cli::cloud::profile::redact_database_url;
    assert_eq!(
        redact_database_url("postgres://user:pass@host:5432/db"),
        "postgres://[REDACTED]@host:5432/db"
    );
    assert_eq!(redact_database_url("no-credentials"), "no-credentials");
    assert_eq!(redact_database_url("user@host"), "user@host");
}
