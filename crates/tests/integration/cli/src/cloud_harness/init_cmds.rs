//! Harness tests for `cloud init` scaffolding in a scratch project root.
//!
//! `GIT_ALLOW_PROTOCOL=none` forces the optional systemprompt-admin clone to
//! fail fast so the tests never touch the network.

use systemprompt_cli::cloud::{self, CloudCommands};

use super::{enter, json_ctx};

struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn chdir_scratch(tmp: &tempfile::TempDir) -> CwdGuard {
    let prev = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(tmp.path()).expect("chdir scratch");
    CwdGuard(prev)
}

#[tokio::test]
async fn init_scaffolds_reruns_and_forces() {
    let _env = enter().await;
    unsafe { std::env::set_var("GIT_ALLOW_PROTOCOL", "none") };
    let tmp = tempfile::tempdir().expect("scratch root");
    let _cwd = chdir_scratch(&tmp);

    cloud::execute(CloudCommands::Init { force: false }, &json_ctx())
        .await
        .expect("initial init");

    let root = tmp.path();
    assert!(root.join(".systemprompt/.gitignore").exists());
    assert!(root.join(".systemprompt/.dockerignore").exists());
    assert!(root.join(".systemprompt/Dockerfile").exists());
    assert!(root.join(".systemprompt/entrypoint.sh").exists());
    assert!(root.join("services/config/config.yaml").exists());
    assert!(root.join("services/agents/assistant.yaml").exists());
    assert!(root.join("services/mcp/systemprompt-admin.yaml").exists());
    assert!(root.join("services/ai/config.yaml").exists());
    assert!(root.join("services/web/templates/page.html").exists());
    assert!(root.join("services/content/blog/welcome/index.md").exists());
    assert!(root.join("services/scheduler/config.yaml").exists());
    assert!(root.join("logs/.gitignore").exists());

    cloud::execute(CloudCommands::Init { force: false }, &json_ctx())
        .await
        .expect("idempotent re-run");

    std::fs::write(root.join("services/marker.txt"), "x").expect("marker");
    cloud::execute(CloudCommands::Init { force: true }, &json_ctx())
        .await
        .expect("forced regeneration");
    assert!(!root.join("services/marker.txt").exists());
    assert!(root.join("services/config/config.yaml").exists());

    unsafe { std::env::remove_var("GIT_ALLOW_PROTOCOL") };
}
