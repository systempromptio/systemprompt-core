//! Harness tests for the local-tenant setup helpers in `cloud::profile`:
//! `get_cloud_user` credential resolution and `handle_local_tenant_setup`'s
//! unreachable-database branch.

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::profile::{get_cloud_user, handle_local_tenant_setup};
use systemprompt_cloud::{CloudPath, get_cloud_paths};

use super::enter;

#[tokio::test]
async fn get_cloud_user_resolves_credentials() {
    let _env = enter().await;
    let user = get_cloud_user().expect("cloud user from credentials");
    assert!(!user.email.is_empty());
}

#[tokio::test]
async fn get_cloud_user_errors_without_credentials() {
    let _env = enter().await;
    std::fs::remove_file(get_cloud_paths().resolve(CloudPath::Credentials))
        .expect("remove credentials");
    let err = get_cloud_user().expect_err("no credentials");
    assert!(err.to_string().contains("credentials"));
}

#[tokio::test]
async fn local_tenant_setup_warns_when_unreachable_and_no_compose() {
    let env = enter().await;
    let user = get_cloud_user().expect("cloud user");
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let profile_path = env
        .root()
        .join(".systemprompt/profiles/local/profile.yaml");

    handle_local_tenant_setup(
        &prompter,
        &user,
        "postgres://nobody:nothing@127.0.0.1:1/void",
        "no-such-compose-tenant",
        &profile_path,
    )
    .await
    .expect("setup tolerates unreachable database without compose file");
}
