//! Full `run_server` lifecycle against a fixture context with zero enabled
//! agents and MCP servers: reconciliation, scheduler init, router activation,
//! readiness signalling, SIGTERM-driven graceful shutdown, and drain.

use std::time::Duration;

use systemprompt_api::services::server::{bind_and_serve, run_server, wait_for_ready};
use tokio::time::sleep;

use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, fixture_db_pool,
};

#[tokio::test]
async fn run_server_reconciles_activates_and_drains_on_sigterm() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let mut config = fixture_config(&b.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];
    let ctx = fixture_app_context_with_config(&pool, config)?;

    let early = bind_and_serve("127.0.0.1:0", None).await?;
    let base = format!("http://{}", early.local_addr());

    let mut server = tokio::spawn(run_server((*ctx).clone(), None, early));

    tokio::select! {
        ready = wait_for_ready(60) => {
            assert!(ready, "run_server never signalled readiness");
        },
        early_exit = &mut server => {
            panic!("run_server exited before readiness: {early_exit:?}");
        },
    }

    let client = reqwest::Client::new();
    let mut activated = false;
    for _ in 0..200 {
        let resp = client.get(format!("{base}/health")).send().await?;
        let body = resp.text().await?;
        if !body.contains("starting") {
            activated = true;
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }
    assert!(activated, "full router was never swapped in");

    let status = std::process::Command::new("kill")
        .args(["-TERM", &std::process::id().to_string()])
        .status()?;
    assert!(status.success(), "kill -TERM failed: {status}");

    let result = tokio::time::timeout(Duration::from_secs(30), server).await??;
    assert!(result.is_ok(), "{result:?}");
    Ok(())
}
