//! Harness tests for `handle_local_tenant_setup`'s unreachable-database
//! branch.

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::cloud::profile::handle_local_tenant_setup;

use super::enter;

#[tokio::test]
async fn local_tenant_setup_warns_when_unreachable_and_no_compose() {
    let env = enter().await;
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let profile_path = env.root().join(".systemprompt/profiles/local/profile.yaml");

    handle_local_tenant_setup(
        &prompter,
        "postgres://nobody:nothing@127.0.0.1:1/void",
        "no-such-compose-tenant",
        &profile_path,
    )
    .await
    .expect("setup tolerates unreachable database without compose file");
}
