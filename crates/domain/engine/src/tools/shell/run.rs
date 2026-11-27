//! Running an authorised shell call: in the foreground, with its output
//! captured, streamed and capped, or as a background job the registry
//! supervises.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use serde_json::json;

use super::capture::{Capture, Live};
use super::process::{self, Ending};
use super::registry::JobStart;
use super::{Call, ShellKind, ShellTool};
use crate::tools::contract::{ToolContext, ToolOutput};
use crate::tools::error::ToolError;
use crate::tools::path::slash;
use crate::tools::truncate::{Keep, truncate_output};

#[derive(Debug, Clone, Copy)]
struct Outcome {
    exit_code: Option<i32>,
    timed_out: bool,
    timeout: Duration,
}

impl ShellTool {
    pub(super) fn run_background(
        &self,
        ctx: &ToolContext,
        call: &Call<'_>,
    ) -> Result<ToolOutput, ToolError> {
        let job = self.jobs.start(&JobStart {
            spec: &self.spec,
            command: call.command,
            cwd: call.cwd,
            session_id: &ctx.session_id,
            data_dir: &ctx.data_dir,
            timeout: call.requested,
        })?;
        let log = slash(&job.output_path);
        let output = format!(
            "Started background job {} in {}.\nIts output is written to {log}.\nYou will be told when \
             it exits. Read that file if you need partial output before then, but do not poll it in \
             a loop; carry on with other work.",
            job.id,
            self.spec.kind.label()
        );
        Ok(
            ToolOutput::new(call.title.clone(), output).with_metadata(json!({
                "background": true,
                "job_id": job.id,
                "output_path": log,
                "shell": self.spec.kind.label(),
            })),
        )
    }

    pub(super) async fn run_foreground(
        &self,
        ctx: &ToolContext,
        call: &Call<'_>,
    ) -> Result<ToolOutput, ToolError> {
        let timeout = call.requested.map_or(self.config.default_timeout, |limit| {
            limit.min(self.config.max_timeout)
        });
        let running = process::start(&self.spec, call.command, call.cwd)?;
        let mut capture = Capture::new(self.config.capture_bytes);
        let mut live = Live::new(ctx.progress.clone(), call.title.clone());
        let ending = running
            .drive(Some(timeout), &ctx.cancel, |chunk| {
                capture.push(chunk);
                live.push(chunk);
            })
            .await;
        live.flush();
        let (exit_code, timed_out) = match ending {
            Ending::Exited(code) => (code, false),
            Ending::TimedOut => (None, true),
            Ending::Cancelled => return Err(ToolError::Aborted),
        };
        let mut text = capture.finish();
        if text.trim().is_empty() {
            "(no output)".clone_into(&mut text);
        }
        let outcome = Outcome {
            exit_code,
            timed_out,
            timeout,
        };
        for note in self.closing_notes(call.command, &text, outcome) {
            text = format!("{}\n\n[{note}]", text.trim_end());
        }
        let capped = truncate_output(
            text,
            self.output,
            Keep::Tail,
            &ctx.data_dir,
            ctx.call_id.as_str(),
        )
        .await;
        Ok(
            ToolOutput::new(call.title.clone(), capped.text).with_metadata(json!({
                "exit_code": exit_code,
                "timed_out": timed_out,
                "shell": self.spec.kind.label(),
                "workdir": slash(call.cwd),
                "description": call.description,
                "truncated": capped.truncated,
                "output_path": capped.spill.as_deref().map(slash),
            })),
        )
    }

    fn closing_notes(&self, command: &str, text: &str, outcome: Outcome) -> Vec<String> {
        let mut notes: Vec<String> = Vec::new();
        if self.spec.kind == ShellKind::WindowsPowerShell
            && (command.contains("&&") || command.contains("||"))
            && text.contains("InvalidEndOfLine")
        {
            notes.push(
                "Windows PowerShell 5.1 does not support && or ||; write `first; if ($?) { second }` \
                 instead."
                    .to_owned(),
            );
        }
        if outcome.timed_out {
            notes.push(format!(
                "The command timed out after {} seconds and was stopped with everything it started. \
                 Rerun with a larger timeout, or with background set to true if it is meant to keep \
                 running.",
                outcome.timeout.as_secs()
            ));
        } else if outcome.exit_code != Some(0) {
            let note = outcome.exit_code.map_or_else(
                || {
                    "The command ended without an exit code (it was terminated by a signal)."
                        .to_owned()
                },
                |code| format!("Exit code: {code}"),
            );
            notes.push(note);
        }
        notes
    }
}
