//! AI-request rendering for trace views.
//!
//! The AI trace is one multi-section document: each section is appended to a
//! [`Transcript`], which becomes the command's single output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::cli::CommandOutput;
use systemprompt_logging::LogLevel;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::{AiRequestInfo, ExecutionStep, TaskInfo};

use crate::commands::infrastructure::logs::shared::level_notice;
use crate::presentation::tables::{ai_requests_table, execution_steps_table, task_info_table};

pub(super) use crate::presentation::tables::truncate_cell as truncate;

#[derive(Debug, Default)]
pub(super) struct Transcript {
    lines: Vec<NoticeLine>,
}

impl Transcript {
    pub(super) fn section(&mut self, title: &str) {
        self.info(format!("── {title} ──"));
    }

    pub(super) fn info(&mut self, text: impl Into<String>) {
        self.lines.push(NoticeLine::new("info", text));
    }

    pub(super) fn error(&mut self, text: impl Into<String>) {
        self.lines.push(NoticeLine::new("error", text));
    }

    pub(super) fn level(&mut self, level: LogLevel, text: impl Into<String>) {
        self.lines.push(level_notice(level, text));
    }

    pub(super) fn block(&mut self, content: &str) {
        for line in content.lines() {
            self.info(format!("  {line}"));
        }
    }

    pub(super) fn into_output(self, title: impl Into<String>) -> CommandOutput {
        CommandOutput::message(self.lines).with_title(title)
    }
}

pub(super) fn task_info(out: &mut Transcript, task_info: &TaskInfo) {
    out.section("TASK");
    out.info(task_info_table(task_info));

    if task_info.status == "failed"
        && let Some(ref error) = task_info.error_message
        && !error.is_empty()
    {
        out.error("Error:");
        out.block(error);
    }
}

pub(super) fn user_input(out: &mut Transcript, input: Option<&String>) {
    if let Some(text) = input {
        out.section("USER INPUT");
        out.info(format!("  {text}"));
    }
}

pub(super) fn agent_response(out: &mut Transcript, response: Option<&String>) {
    if let Some(text) = response {
        out.section("AGENT RESPONSE");
        out.block(text);
    }
}

pub(super) fn execution_steps(out: &mut Transcript, steps: &[ExecutionStep]) {
    if steps.is_empty() {
        return;
    }

    out.section("EXECUTION STEPS");
    out.info(execution_steps_table(steps));

    for step in steps {
        if step.status == "failed"
            && let Some(ref error) = step.error_message
            && !error.is_empty()
        {
            let step_type = step.step_type.clone().unwrap_or_else(|| "step".to_owned());
            out.error(format!("  {step_type} failed:"));
            out.block(error);
        }
    }
}

pub(super) fn ai_requests(out: &mut Transcript, requests: &[AiRequestInfo]) {
    if requests.is_empty() {
        return;
    }

    out.section("AI REQUESTS");
    out.info(ai_requests_table(requests));
}
