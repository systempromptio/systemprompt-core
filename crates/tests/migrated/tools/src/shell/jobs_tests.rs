//! Timeouts and background commands: what is reported, when, and how a
//! command is stopped.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;
use systemprompt_engine::tools::{
    BackgroundJobs, JobInfo, JobListener, JobReport, JobState, OutputLimits, ShellConfig, Tool,
    ToolError,
};
use systemprompt_identifiers::engine::JobId;

use super::posix::{plain, run_input, tool};
use crate::support::{Gate, Project};

#[derive(Debug, Default)]
struct Reports(Mutex<Vec<JobReport>>);

impl JobListener for Reports {
    fn finished(&self, report: &JobReport) {
        self.0.lock().unwrap().push(report.clone());
    }
}

impl Reports {
    fn all(&self) -> Vec<JobReport> {
        self.0.lock().unwrap().clone()
    }
}

struct Jobs {
    project: Project,
    jobs: BackgroundJobs,
    reports: Arc<Reports>,
}

impl Jobs {
    fn new() -> Self {
        let jobs = BackgroundJobs::new();
        let reports = Arc::new(Reports::default());
        jobs.set_listener(Arc::<Reports>::clone(&reports));
        Self {
            project: Project::new(),
            jobs,
            reports,
        }
    }

    async fn start(&self, command: &str, timeout: Option<u64>) -> (JobId, String) {
        let shell = tool(ShellConfig::default(), OutputLimits::default(), &self.jobs);
        let mut input = json!({"command": command, "background": true});
        if let Some(timeout) = timeout {
            input["timeout"] = json!(timeout);
        }
        let out = self.project.call(&shell, input).await.ok();
        let id = JobId::parse(out.metadata["job_id"].as_str().unwrap()).unwrap();
        (id, out.output)
    }

    async fn finished(&self, id: &JobId) -> JobInfo {
        for _ in 0..600 {
            if let Some(info) = self
                .jobs
                .get(id)
                .filter(|info| info.state != JobState::Running)
            {
                return info;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("job {id} did not finish");
    }

    async fn reports_for(&self, id: &JobId) -> Vec<JobReport> {
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.reports
            .all()
            .into_iter()
            .filter(|report| &report.job.id == id)
            .collect()
    }
}

#[tokio::test]
#[ignore = "parity ledger: a foreground command that outlives its timeout is killed, not moved to the background"]
async fn a_foreground_timeout_moves_the_command_to_the_background() {
    let project = Project::new();
    let out = run_input(
        &project,
        json!({"command": "sleep 3; echo late", "timeout": 1}),
        Gate::default(),
    )
    .await
    .ok();
    assert_eq!(out.metadata["background"], true);
}

#[tokio::test]
async fn a_fast_command_returns_its_result_directly() {
    let project = Project::new();
    let out = run_input(
        &project,
        json!({"command": "echo quick", "timeout": 30}),
        Gate::default(),
    )
    .await
    .ok();
    assert_eq!(out.output, "quick\n");
    assert_eq!(out.metadata["timed_out"], false);
}

#[tokio::test]
#[ignore = "parity ledger: a zero timeout is refused instead of meaning the default"]
async fn a_zero_timeout_means_the_default_timeout() {
    let project = Project::new();
    let out = run_input(
        &project,
        json!({"command": "echo ok", "timeout": 0}),
        Gate::default(),
    )
    .await
    .ok();
    assert_eq!(out.output, "ok\n");
}

#[tokio::test]
async fn a_background_command_reports_its_id_once_and_its_result_once() {
    let fixture = Jobs::new();
    let (id, output) = fixture.start("echo hello; exit 3", None).await;
    assert!(
        output.starts_with(&format!("Started background job {id} in a POSIX shell.")),
        "{output}"
    );
    let done = fixture.finished(&id).await;
    assert_eq!((done.state, done.exit_code), (JobState::Exited, Some(3)));
    let reports = fixture.reports_for(&id).await;
    assert_eq!(reports.len(), 1);
    assert!(reports[0].notice().contains("exited with code 3"));
    assert!(reports[0].tail.contains("hello"));
}

#[tokio::test]
async fn a_silent_command_that_ends_at_once_is_still_recorded() {
    let fixture = Jobs::new();
    let (id, _) = fixture.start("true", None).await;
    let done = fixture.finished(&id).await;
    assert_eq!(done.exit_code, Some(0));
    assert_eq!(fixture.jobs.read(&id, 1024).unwrap(), "");
    assert!(done.output_path.exists());
    assert_eq!(fixture.reports_for(&id).await.len(), 1);
}

#[tokio::test]
async fn a_command_ended_by_an_outside_signal_settles_without_an_exit_code() {
    let fixture = Jobs::new();
    let (id, _) = fixture.start("kill -9 $$", None).await;
    let done = fixture.finished(&id).await;
    assert_eq!(done.exit_code, None);
    assert_eq!(fixture.reports_for(&id).await.len(), 1);
}

#[tokio::test]
#[ignore = "parity ledger: the signal that ended a background command is not reported"]
async fn the_signal_that_ended_a_command_is_reported() {
    let fixture = Jobs::new();
    let (id, _) = fixture.start("kill -9 $$", None).await;
    fixture.finished(&id).await;
    let reports = fixture.reports_for(&id).await;
    assert!(reports[0].notice().contains("SIGKILL") || reports[0].notice().contains("signal 9"));
}

#[tokio::test]
async fn a_background_command_can_be_stopped_by_its_id_and_its_record_is_kept() {
    let fixture = Jobs::new();
    let (id, _) = fixture.start("sleep 30", None).await;
    assert!(fixture.jobs.kill(&id));
    let done = fixture.finished(&id).await;
    assert_eq!(done.state, JobState::Killed);
    assert_eq!(
        fixture.jobs.get(&id).map(|info| info.state),
        Some(JobState::Killed)
    );
    let listed = fixture.jobs.list(&done.session_id);
    assert_eq!(listed, [fixture.jobs.get(&id).unwrap()]);
}

#[tokio::test]
async fn a_timed_out_background_command_is_stopped_and_reported_once() {
    let fixture = Jobs::new();
    let started = Instant::now();
    let (id, _) = fixture.start("sleep 30", Some(1)).await;
    let done = fixture.finished(&id).await;
    assert_eq!(done.state, JobState::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(10));
    let reports = fixture.reports_for(&id).await;
    assert_eq!(reports.len(), 1);
    assert!(reports[0].notice().contains("timed out and was stopped"));
}

#[tokio::test]
async fn a_timed_out_command_is_asked_to_stop_before_it_is_killed() {
    let fixture = Jobs::new();
    let (id, _) = fixture
        .start(
            "trap 'echo stopping; exit 0' TERM; sleep 30 & wait",
            Some(1),
        )
        .await;
    fixture.finished(&id).await;
    assert!(fixture.jobs.read(&id, 4096).unwrap().contains("stopping"));
}

#[tokio::test]
async fn a_stubborn_command_that_ignores_the_polite_stop_still_ends() {
    let fixture = Jobs::new();
    let started = Instant::now();
    let (id, _) = fixture.start("trap '' TERM; sleep 30", Some(1)).await;
    let done = fixture.finished(&id).await;
    assert_eq!(done.state, JobState::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(12));
}

#[tokio::test]
async fn stopping_a_command_also_stops_the_processes_it_started() {
    let fixture = Jobs::new();
    let marker = fixture.project.root.join("grandchild-alive");
    let script = format!("(sleep 2; touch {}) & sleep 30", marker.display());
    let (id, _) = fixture.start(&script, None).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(fixture.jobs.kill(&id));
    fixture.finished(&id).await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(!marker.exists());
}

#[tokio::test]
async fn the_output_file_keeps_everything_and_reads_of_its_end_never_split_a_character() {
    let fixture = Jobs::new();
    let (id, _) = fixture
        .start(
            "i=0; while [ $i -lt 3000 ]; do echo \"line $i é\"; i=$((i+1)); done",
            None,
        )
        .await;
    let done = fixture.finished(&id).await;
    let whole = std::fs::read_to_string(&done.output_path).unwrap();
    assert!(whole.starts_with("line 0 é\n"));
    assert!(whole.ends_with("line 2999 é\n"));
    for size in [7, 100, 1001] {
        let tail = fixture.jobs.read(&id, size).unwrap();
        assert!(whole.ends_with(&tail));
        assert!(
            tail.is_empty() || tail.starts_with("line"),
            "{size}: {tail:?}"
        );
    }
}

#[tokio::test]
async fn a_command_whose_directory_is_missing_never_starts_or_writes_a_file() {
    let fixture = Jobs::new();
    let shell = tool(
        ShellConfig::default(),
        OutputLimits::default(),
        &fixture.jobs,
    );
    let called = fixture
        .project
        .call(
            &shell,
            json!({"command": "touch x", "background": true, "workdir": "absent"}),
        )
        .await;
    let session = called.gate.asked().len();
    assert!(matches!(called.err(), ToolError::InvalidInput { .. }));
    assert_eq!(session, 0);
    assert!(!fixture.project.root.join(".data/jobs").exists());
}

#[tokio::test]
#[ignore = "parity ledger: the line count of a background command's output is not tracked"]
async fn the_line_count_of_a_background_command_follows_what_was_written() {
    let fixture = Jobs::new();
    let (id, _) = fixture.start("printf 'a\\nb\\nc'", None).await;
    let done = fixture.finished(&id).await;
    let notice = fixture.reports_for(&id).await[0].notice();
    assert!(
        notice.contains("2 lines") || notice.contains("3 lines"),
        "{notice} {done:?}"
    );
}

#[tokio::test]
async fn removing_every_command_of_a_session_continues_past_finished_ones() {
    let fixture = Jobs::new();
    let (first, _) = fixture.start("true", None).await;
    fixture.finished(&first).await;
    let (second, _) = fixture.start("sleep 30", None).await;
    let session = fixture.jobs.get(&second).unwrap().session_id;
    assert_eq!(fixture.jobs.kill_all(&session), 1);
    assert_eq!(fixture.jobs.list(&session), Vec::<JobInfo>::new());
}

#[tokio::test]
async fn a_call_stopped_before_it_started_never_launches_its_command() {
    let project = Project::new();
    let marker = project.root.join("ran");
    let ctx = project.context(Arc::new(Gate::default()));
    ctx.cancel.cancel();
    let result = plain()
        .run(
            json!({"command": format!("touch {}", marker.display())}),
            &ctx,
        )
        .await;
    assert!(matches!(result, Err(ToolError::Aborted)));
    assert!(!marker.exists());
}

#[tokio::test]
#[ignore = "parity ledger: an interrupted command's result names neither its saved output nor its shell"]
async fn an_interrupted_command_names_its_saved_output_and_shell() {
    let project = Project::new();
    let ctx = project.context(Arc::new(Gate::default()));
    let trigger = ctx.cancel.clone();
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        trigger.cancel();
    });
    let result = plain()
        .run(json!({"command": "echo partial; sleep 30"}), &ctx)
        .await;
    stopper.await.unwrap();
    let out = result.unwrap();
    assert!(out.output.contains("partial"));
    assert!(out.metadata["output_path"].is_string());
}

#[tokio::test]
async fn the_job_directory_is_recreated_when_it_was_removed() {
    let fixture = Jobs::new();
    let (first, _) = fixture.start("true", None).await;
    fixture.finished(&first).await;
    std::fs::remove_dir_all(fixture.project.root.join(".data/jobs")).unwrap();
    let (second, _) = fixture.start("echo again", None).await;
    fixture.finished(&second).await;
    assert_eq!(fixture.jobs.read(&second, 64).unwrap(), "again\n");
}
