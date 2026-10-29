//! `infra services serve` refuses a port held by a process it cannot verify
//! as a systemprompt API server, before it binds or builds anything.
//!
//! The test process itself holds an ephemeral port and the profile names it,
//! so the holder is real and never ours to signal: every path asserted here
//! ends in the refusal, and none reaches the bind or the stop.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::net::TcpListener;
use std::sync::Arc;

use futures::StreamExt;
use systemprompt_cli::infrastructure::services::serve::{ServeOptions, execute_with_events};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_config::LoadedProfile;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use systemprompt_traits::StartupEvent;

struct Held {
    _listener: TcpListener,
    _dir: tempfile::TempDir,
    port: u16,
    profile: Arc<LoadedProfile>,
}

fn held_port() -> Held {
    let bootstrap = ensure_test_bootstrap();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let text = std::fs::read_to_string(&bootstrap.profile_path).expect("profile");
    let mut yaml: serde_yaml::Value = serde_yaml::from_str(&text).expect("yaml");
    yaml["server"]["port"] = serde_yaml::Value::from(port);
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("profile.yaml");
    std::fs::write(&path, serde_yaml::to_string(&yaml).expect("ser")).expect("write");
    let profile = LoadedProfile::from_path(&path).expect("loads");
    Held {
        _listener: listener,
        _dir: dir,
        port,
        profile,
    }
}

fn ctx(held: &Held, interactive: bool) -> CommandContext {
    let bootstrap = ensure_test_bootstrap();
    CommandContext::new(
        CliConfig::new().with_interactive(interactive),
        EnvOverrides::default(),
    )
    .with_boot(Some(bootstrap.boot.clone()))
    .with_profile(Some(Arc::clone(&held.profile)))
}

const OPTIONS: ServeOptions = ServeOptions {
    foreground: true,
    kill_port_process: false,
    run_migrations: false,
};

fn refused_unverified(result: anyhow::Result<String>, port: u16) {
    let error = result.expect_err("an unverified holder is refused");
    let message = format!("{error:#}");
    assert!(
        message.contains(&format!(
            "Port {port} is held by PID {}",
            std::process::id()
        )),
        "{message}"
    );
    assert!(
        message.contains("not a verified systemprompt API server"),
        "{message}"
    );
}

#[tokio::test]
async fn an_unverified_holder_is_refused_without_kill_flag() {
    let held = held_port();
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    refused_unverified(
        execute_with_events(&prompter, OPTIONS, &ctx(&held, false), None).await,
        held.port,
    );
}

#[tokio::test]
async fn kill_flag_non_interactive_declines_to_signal_an_unverified_holder() {
    let held = held_port();
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let options = ServeOptions {
        kill_port_process: true,
        ..OPTIONS
    };
    refused_unverified(
        execute_with_events(&prompter, options, &ctx(&held, false), None).await,
        held.port,
    );
}

#[tokio::test]
async fn an_operator_who_declines_leaves_the_holder_and_events_report_the_conflict() {
    let held = held_port();
    let prompter = ScriptedPrompter::new(["n"]);
    let options = ServeOptions {
        kill_port_process: true,
        ..OPTIONS
    };
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    refused_unverified(
        execute_with_events(&prompter, options, &ctx(&held, true), Some(&tx)).await,
        held.port,
    );
    drop(tx);
    let mut conflict = None;
    while let Some(event) = rx.next().await {
        if let StartupEvent::PortConflict { port, pid } = event {
            conflict = Some((port, pid));
        }
    }
    assert_eq!(conflict, Some((held.port, std::process::id())));
}
