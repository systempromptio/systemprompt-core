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

// Nested inside `server_run_loop` so the test path matches the
// `scheduler-services-db` serialisation filter: `run_server` sweeps the whole
// `services` table, which races any test holding a seeded row.
mod agent_failure {
    // `run_server` when a required agent cannot be started.
    //
    // The existing lifecycle test boots against a profile with zero enabled
    // agents, so `reconcile_agents` returns immediately and the whole failure
    // ladder beneath it — `start_enabled_agents`, `handle_failed_agents`, the
    // cleanup-and-retry in `enforce_clean_agent_state`, and `runner::fail_phase` —
    // never runs. This profile declares one enabled agent whose binary does not
    // exist, which drives exactly that ladder.
    //
    // The agent is declared in an *isolated* bootstrap rather than reusing the
    // shared messaging fixture: `enforce_clean_agent_state` deletes and re-creates
    // the agent's `services` row, which would race any concurrently running test
    // that depends on that row.
    //
    // Agents are a hard dependency of the API: a server that came up serving
    // traffic while a required agent was dead would strand every request that
    // needs it, so the contract is that startup fails loudly instead.

    use std::sync::OnceLock;
    use std::time::Duration;

    use systemprompt_api::services::server::{bind_and_serve, run_server};
    use systemprompt_test_fixtures::{
        TestBootstrap, fixture_app_context_with_config, fixture_config, fixture_db_pool,
        init_isolated_bootstrap,
    };

    const AGENT: &str = "unstartable_fixture_agent";
    const AGENT_PORT: u16 = 4931;

    fn services_config() -> String {
        format!(
            r#"agents:
  {AGENT}:
    name: {AGENT}
    port: {AGENT_PORT}
    endpoint: http://127.0.0.1:{AGENT_PORT}
    enabled: true
    card:
      protocolVersion: "0.3.0"
      displayName: Unstartable Fixture Agent
      description: Declared enabled with no binary, so startup must fail.
      version: "1.0.0"
    metadata: {{}}
    oauth:
      required: false
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
    "#
        )
    }

    static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

    fn boot() -> &'static TestBootstrap {
        BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", &services_config()))
    }

    #[tokio::test]
    async fn run_server_fails_when_a_required_agent_cannot_start() -> anyhow::Result<()> {
        let b = boot();
        let pool = fixture_db_pool(&b.database_url).await?;
        let mut config = fixture_config(&b.database_url);
        config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];
        let ctx = fixture_app_context_with_config(&pool, config)?;

        let early = bind_and_serve("127.0.0.1:0", None).await?;

        let result = tokio::time::timeout(
            Duration::from_secs(120),
            run_server((*ctx).clone(), None, early),
        )
        .await
        .map_err(|_e| anyhow::anyhow!("run_server hung instead of failing the agent phase"))?;

        let err = result.expect_err("an agent that cannot start must fail server startup");
        let rendered = err.to_string();

        // `run_agents_phase` returns the underlying error and sends the
        // phase-prefixed wording to the event channel, so the returned error is the
        // reconciler's own FATAL text.
        assert!(
            rendered.contains("failed to start after retry"),
            "the failure must say the retry was already attempted: {rendered}"
        );
        assert!(
            rendered.contains(AGENT),
            "the operator has to be told which agent could not start: {rendered}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn the_agent_failure_is_reported_through_the_startup_event_channel() -> anyhow::Result<()>
    {
        let b = boot();
        let pool = fixture_db_pool(&b.database_url).await?;
        let ctx = fixture_app_context_with_config(&pool, fixture_config(&b.database_url))?;

        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let early = bind_and_serve("127.0.0.1:0", None).await?;

        let result = tokio::time::timeout(
            Duration::from_secs(120),
            run_server((*ctx).clone(), Some(tx), early),
        )
        .await
        .map_err(|_e| anyhow::anyhow!("run_server hung instead of failing the agent phase"))?;
        assert!(result.is_err(), "startup must fail");

        // The CLI renders these events as the boot progress display, so a failure
        // that never reaches the channel is a failure the operator watches succeed.
        let mut saw_fatal = false;
        while let Ok(event) = rx.try_recv() {
            if format!("{event:?}").contains("fatal: true") {
                saw_fatal = true;
            }
        }
        assert!(
            saw_fatal,
            "the agent phase failure must be announced as fatal on the startup channel"
        );
        Ok(())
    }
}

// Nested inside `server_run_loop` so the test path matches the
// `scheduler-services-db` serialisation filter: `run_server` sweeps the whole
// `services` table, which races any test holding a seeded row.
mod mcp_failure {
    // `run_server` when a required MCP server cannot be started.
    //
    // The existing lifecycle test boots with zero managed MCP servers, so the MCP
    // phase finds nothing to reconcile and its failure handling never runs. Only
    // *internal* servers are managed (`get_managed_servers` filters on
    // `is_internal`), so an external fixture entry is not enough — this profile
    // declares an internal server naming an extension that does not exist, which
    // fails the phase while the orchestrator is being built.
    //
    // Reaching `handle_missing_servers` specifically (rather than this earlier
    // failure) would need a *loadable but unstartable* server, i.e. a real
    // `extensions/<name>/manifest.yaml` fixture; that is recorded as future work
    // rather than faked here.
    //
    // Agents depend on MCP tools, so a server that came up with a required MCP
    // server missing would leave every tool call failing at runtime. The contract
    // is that startup fails instead.

    use std::sync::OnceLock;
    use std::time::Duration;

    use systemprompt_api::services::server::{bind_and_serve, run_server};
    use systemprompt_test_fixtures::{
        TestBootstrap, fixture_app_context_with_config, fixture_config, fixture_db_pool,
        init_isolated_bootstrap,
    };

    const MCP_NAME: &str = "fixture_unstartable_mcp";

    fn services_config() -> String {
        format!(
            r#"mcp_servers:
  {MCP_NAME}:
    type: internal
    binary: no-such-mcp-binary
    package: fixture
    port: 5987
    enabled: true
    display_in_web: false
    oauth:
      required: false
      scopes: []
      audience: mcp
      client_id: null
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
    "#
        )
    }

    static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

    fn boot() -> &'static TestBootstrap {
        BOOT.get_or_init(|| init_isolated_bootstrap("http://127.0.0.1", &services_config()))
    }

    #[tokio::test]
    async fn run_server_fails_when_a_required_mcp_server_cannot_start() -> anyhow::Result<()> {
        let b = boot();
        let pool = fixture_db_pool(&b.database_url).await?;
        let ctx = fixture_app_context_with_config(&pool, fixture_config(&b.database_url))?;

        let early = bind_and_serve("127.0.0.1:0", None).await?;

        let result = tokio::time::timeout(
            Duration::from_secs(120),
            run_server((*ctx).clone(), None, early),
        )
        .await
        .map_err(|_e| anyhow::anyhow!("run_server hung instead of failing the MCP phase"))?;

        let err = result.expect_err("an unstartable MCP server must fail server startup");
        let rendered = err.to_string();

        // A typo'd or removed MCP extension must stop the boot outright: coming up
        // without it would leave every tool call failing at runtime instead.
        assert!(
            rendered.contains("no-such-mcp-binary"),
            "the operator has to be told which extension could not be resolved: {rendered}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn the_mcp_failure_is_announced_as_fatal_on_the_startup_channel() -> anyhow::Result<()> {
        let b = boot();
        let pool = fixture_db_pool(&b.database_url).await?;
        let ctx = fixture_app_context_with_config(&pool, fixture_config(&b.database_url))?;

        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let early = bind_and_serve("127.0.0.1:0", None).await?;

        let result = tokio::time::timeout(
            Duration::from_secs(120),
            run_server((*ctx).clone(), Some(tx), early),
        )
        .await
        .map_err(|_e| anyhow::anyhow!("run_server hung instead of failing the MCP phase"))?;
        assert!(result.is_err(), "startup must fail");

        // The phase never completes, so the boot display must not show the MCP
        // phase as finished.
        let mut saw_mcp_completion = false;
        while let Ok(event) = rx.try_recv() {
            let rendered = format!("{event:?}");
            if rendered.contains("PhaseCompleted") && rendered.contains("McpServers") {
                saw_mcp_completion = true;
            }
        }
        assert!(
            !saw_mcp_completion,
            "a phase that failed must never be announced as completed"
        );
        Ok(())
    }
}
