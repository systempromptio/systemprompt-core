//! The shell tool and background commands: permission questions, working
//! directory, output, exit status, timeouts, background jobs and the
//! processes it starts. POSIX hosts run the commands through /bin/sh.

#[cfg(unix)]
mod jobs_tests;
#[cfg(unix)]
mod output_tests;
#[cfg(unix)]
mod permission_tests;
#[cfg(unix)]
mod process_tests;
mod selection_tests;

#[cfg(unix)]
pub(crate) mod posix {
    use serde_json::{Value, json};
    use systemprompt_engine::tools::{
        BackgroundJobs, OutputLimits, ShellConfig, ShellKind, ShellSpec, ShellTool,
    };

    use crate::support::{Called, Gate, Project};

    pub(crate) fn tool(
        config: ShellConfig,
        output: OutputLimits,
        jobs: &BackgroundJobs,
    ) -> ShellTool {
        let spec = ShellSpec {
            path: "/bin/sh".into(),
            kind: ShellKind::Posix,
        };
        ShellTool::with_spec(spec, config, output, jobs.clone())
    }

    pub(crate) fn plain() -> ShellTool {
        tool(
            ShellConfig::default(),
            OutputLimits::default(),
            &BackgroundJobs::new(),
        )
    }

    pub(crate) async fn run(project: &Project, command: &str) -> Called {
        project.call(&plain(), json!({"command": command})).await
    }

    pub(crate) async fn run_input(project: &Project, input: Value, gate: Gate) -> Called {
        project.call_with(&plain(), input, gate).await
    }
}
