use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use systemprompt_engine::tools::{
    BackgroundJobs, JobInfo, JobListener, JobReport, JobState, OutputLimits, ShellConfig,
    ShellKind, ShellSpec, ShellTool, Tool, ToolContext,
};
use systemprompt_identifiers::engine::JobId;
use systemprompt_traits::BackgroundTasks;

use crate::tools::support::recorded;

#[derive(Debug, Default)]
struct Collect(Mutex<Vec<JobReport>>);

impl JobListener for Collect {
    fn finished(&self, report: &JobReport) {
        self.0.lock().unwrap().push(report.clone());
    }
}

impl Collect {
    fn reports(&self) -> Vec<JobReport> {
        self.0.lock().unwrap().clone()
    }
}

fn sh(jobs: &BackgroundJobs) -> ShellTool {
    let spec = ShellSpec {
        path: "/bin/sh".into(),
        kind: ShellKind::Posix,
    };
    ShellTool::with_spec(
        spec,
        ShellConfig::default(),
        OutputLimits::default(),
        jobs.clone(),
    )
}

async fn start(jobs: &BackgroundJobs, ctx: &ToolContext, command: &str) -> JobId {
    let out = sh(jobs)
        .run(json!({"command": command, "background": true}), ctx)
        .await
        .unwrap();
    assert_eq!(out.metadata["background"], true);
    JobId::parse(out.metadata["job_id"].as_str().unwrap()).unwrap()
}

async fn wait_for(jobs: &BackgroundJobs, id: &JobId) -> JobInfo {
    for _ in 0..400 {
        if let Some(info) = jobs.get(id).filter(|info| info.state != JobState::Running) {
            return info;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("job {id} did not finish");
}

#[tokio::test]
async fn runs_reports_and_reads_output() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let jobs = BackgroundJobs::new();
    let collect = Arc::new(Collect::default());
    jobs.set_listener(Arc::<Collect>::clone(&collect));
    let id = start(&jobs, &ctx, "echo hello; exit 2").await;
    assert!(id.as_str().starts_with("job_"));
    let done = wait_for(&jobs, &id).await;
    assert_eq!((done.state, done.exit_code), (JobState::Exited, Some(2)));
    assert!(jobs.read(&id, 1024).unwrap().contains("hello"));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let reports = collect.reports();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].tail.contains("hello"));
    assert!(reports[0].notice().contains("exited with code 2"));
    assert_eq!(jobs.list(&ctx.session_id).len(), 1);
}

#[tokio::test]
async fn kill_by_id_tells_the_listener() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let jobs = BackgroundJobs::new();
    let collect = Arc::new(Collect::default());
    jobs.set_listener(Arc::<Collect>::clone(&collect));
    let id = start(&jobs, &ctx, "sleep 30").await;
    assert!(jobs.kill(&id));
    let done = wait_for(&jobs, &id).await;
    assert_eq!(done.state, JobState::Killed);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(collect.reports()[0].notice().contains("was stopped"));
    assert!(!jobs.kill(&id));
}

#[tokio::test]
async fn kill_all_is_silent() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let jobs = BackgroundJobs::new();
    let collect = Arc::new(Collect::default());
    jobs.set_listener(Arc::<Collect>::clone(&collect));
    start(&jobs, &ctx, "sleep 30").await;
    assert_eq!(jobs.kill_all(&ctx.session_id), 1);
    assert_eq!(jobs.list(&ctx.session_id), Vec::<JobInfo>::new());
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(collect.reports(), Vec::<JobReport>::new());
}

#[tokio::test]
async fn shutting_down_the_task_set_stops_its_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let tasks = BackgroundTasks::new();
    let jobs = BackgroundJobs::with_tasks(tasks.clone());
    let id = start(&jobs, &ctx, "sleep 30").await;
    let report = tasks.shutdown(Duration::from_secs(10)).await;
    assert!(report.is_drained());
    assert_eq!(jobs.get(&id).map(|info| info.state), Some(JobState::Killed));
}
