//! One scenario on one protocol, driven through the harness's headless run.
//!
//! The run writes its events to a sink that hands each line to a watcher. The
//! watcher performs the scenario's actions on the events they name (the
//! `index`-th ask, the first text of the `index`-th model response, the
//! `index`-th idle) through the run's control feed, records each with the
//! number of model requests the instance had received, and closes the feed at
//! an idle once no idle action is left (after a revert, once the session
//! shows it, so the revert's store change is on the stream before the run
//! ends).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Context as _;
use serde_json::Value;
use systemprompt_harness::command::{
    Answers, EventWriter, LoginRequest, PermissionMode, RunReport, RunRequest, login, read_token,
    run_controlled,
};
use systemprompt_parity_compare::Mark;
use systemprompt_scripted_instance::{
    Aside, ManifestFixture, ModelFixture, ModelScript, PluginFixture, RequestRecord,
    ScriptedInstance, VerdictScript,
};
use systemprompt_traits::CancellationToken;
use tokio::sync::mpsc;

use crate::host::host;
use crate::scenario::Scenario;
use crate::watch::{LineSink, Watcher};

pub const MODEL: &str = "scripted-model";
/// The plugin that owns the governance hooks, so the harness asks for a
/// verdict on every call its rules allow.
pub const GOVERNANCE_PLUGIN: &str = "governance";
const RUN_TIMEOUT: Duration = Duration::from_secs(60);
const SYNC_TIMEOUT: Duration = Duration::from_secs(30);
const FIXTURE_MTIME: Duration = Duration::from_secs(1_700_000_000);

/// What one run left behind.
#[derive(Debug)]
pub struct RunOutput {
    pub report: RunReport,
    pub events: Vec<Value>,
    pub marks: Vec<Mark>,
    pub requests: Vec<RequestRecord>,
    pub requests_before_run: usize,
    pub unconsumed_turns: usize,
    pub title_calls: usize,
    /// Processes still running in the workspace after the run.
    pub left_running: usize,
    pub workspace: PathBuf,
    pub turns: String,
    pub dir: tempfile::TempDir,
}

fn copy_fixture(from: &Path, to: &Path) -> anyhow::Result<()> {
    let instant = SystemTime::UNIX_EPOCH + FIXTURE_MTIME;
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_fixture(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
            std::fs::OpenOptions::new()
                .write(true)
                .open(&target)?
                .set_modified(instant)?;
        }
    }
    Ok(())
}

/// Makes the workspace a repository of its own, so the harness's snapshots
/// are taken of it and not of a repository the scratch directory sits in.
/// `.git` is not part of the compared tree.
async fn own_repository(workspace: &Path) -> anyhow::Result<()> {
    let status = tokio::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(workspace)
        .status()
        .await?;
    anyhow::ensure!(
        status.success(),
        "git init failed in {}",
        workspace.display()
    );
    Ok(())
}

/// How many processes still have the workspace as their working directory,
/// after giving a killed process tree a moment to go.
async fn left_running(workspace: &Path) -> usize {
    let mut count = 0;
    for _ in 0..20 {
        let listed = tokio::process::Command::new("lsof")
            .args(["-a", "-d", "cwd", "-Fn", "+D"])
            .arg(workspace)
            .output()
            .await;
        count = listed.map_or(0, |out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|line| line.starts_with('p'))
                .count()
        });
        if count == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    count
}

async fn instance(
    scenario: &Scenario,
    protocol: &str,
    turns: &str,
) -> anyhow::Result<ScriptedInstance> {
    let model = if protocol == "chat" {
        ModelFixture::chat(MODEL)
    } else {
        ModelFixture::messages(MODEL)
    };
    let verdict = match scenario.verdicts()? {
        Some(json) => VerdictScript::from_json(&json)?,
        None => VerdictScript::allow_all(),
    };
    Ok(ScriptedInstance::builder()
        .manifest(
            ManifestFixture::minimal().with_plugin(PluginFixture::governance(GOVERNANCE_PLUGIN)),
        )
        .models([model])
        .script(ModelScript::from_json(turns)?)
        .verdict(verdict)
        .title_reply(&scenario.title_reply)
        .start()
        .await?)
}

/// Runs `scenario` on `protocol` under a fresh directory in `scratch`.
pub async fn drive(
    scenario: &Scenario,
    protocol: &str,
    scratch: &Path,
) -> anyhow::Result<RunOutput> {
    std::fs::create_dir_all(scratch)?;
    let dir = tempfile::tempdir_in(scratch)?;
    let turns = scenario.turns(protocol)?;
    let instance = instance(scenario, protocol, &turns).await?;
    let host = host(dir.path());
    let root = dir.path().join("root");
    std::fs::create_dir_all(root.join("config"))?;
    std::fs::write(
        root.join("config").join("harness.json"),
        scenario.settings().to_string(),
    )?;
    let workspace = dir.path().join("workspace");
    copy_fixture(&scenario.dir.join("workspace"), &workspace)?;
    own_repository(&workspace).await?;

    let token = read_token(&mut instance.pat().as_str().as_bytes())?;
    let signed = login(
        &host,
        LoginRequest {
            instance: instance.base_url().to_owned(),
            token,
            fingerprint: None,
            data_root: Some(root.clone()),
        },
    )
    .await;
    anyhow::ensure!(
        signed.status.is_success(),
        "login failed: {}",
        signed.encode()?
    );
    let requests_before_run = instance.requests().len();

    let request = RunRequest {
        prompts: vec![scenario.messages.first().cloned().unwrap_or_default()],
        workspace: workspace.clone(),
        permission_mode: PermissionMode::parse(&scenario.mode).context("permission mode")?,
        answers: Answers::default(),
        model: None,
        agent: None,
        data_root: Some(root),
        sync_timeout: SYNC_TIMEOUT,
        timeout: Some(RUN_TIMEOUT),
    };
    let (controls, feed) = mpsc::unbounded_channel();
    let (lines, received) = mpsc::unbounded_channel();
    let mut out = EventWriter::new(LineSink {
        pending: Vec::new(),
        lines,
    });
    let cancel = CancellationToken::new();
    let mut watcher = Watcher::new(scenario, &instance, controls);
    let (report, ()) = tokio::join!(
        run_controlled(&host, request, (&cancel, feed), &mut out),
        watcher.watch(received),
    );
    let left_running = left_running(&workspace).await;
    let requests = instance.requests();
    let title_calls = requests
        .iter()
        .filter(|record| record.purpose.as_deref() == Some(Aside::TITLE))
        .count();
    Ok(RunOutput {
        left_running,
        report,
        events: watcher.events,
        marks: watcher.marks,
        requests,
        requests_before_run,
        unconsumed_turns: instance.unconsumed_turns(),
        title_calls,
        workspace,
        turns,
        dir,
    })
}
