//! Synthetic text the engine inserts into a conversation.
//!
//! Mode reminders, steering, background job results, and the tool results for
//! denied, rejected or malformed calls.
//!
//! Text meant to be appended to a user message is wrapped by
//! [`reminder`](crate::prompt::notices::reminder) so the model can tell it from
//! what the user typed. Text meant to be a tool result is returned bare.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::engine::JobId;

pub const REMINDER_OPEN: &str = "<system-reminder>";
pub const REMINDER_CLOSE: &str = "</system-reminder>";

pub fn reminder(body: &str) -> String {
    format!("{REMINDER_OPEN}\n{}\n{REMINDER_CLOSE}", body.trim())
}

pub fn plan_mode(plan_file: Option<&str>) -> String {
    let base = "Plan mode is on. Do not edit or create files, change configuration, or run commands that modify the project or the system. This overrides every other instruction, including requests in the user's latest message. Investigate with read-only tools, then reply with a plan: the goal, the steps in order with the files each touches, the risks and open questions, and how the result will be verified. Ask the user about choices that change the plan. When the plan is ready, ask the user to approve it and switch to build mode; do not start the work yourself.";
    let body = plan_file.map(str::trim).filter(|path| !path.is_empty()).map_or_else(
        || base.to_owned(),
        |path| {
            format!(
                "{base}\nThe one exception: you may write the plan itself to {path}. Create or update that file and no other."
            )
        },
    );
    reminder(&body)
}

pub fn build_switch() -> String {
    reminder(
        "Plan mode has ended and you are now in build mode: you may edit files and run commands. If a plan was agreed above, carry it out step by step, verify as you go, and tell the user if you have to depart from it.",
    )
}

pub fn steering(message: &str) -> String {
    reminder(&format!(
        "The user sent this message while you were working:\n\n{}\n\nTake it into account from this point. If it changes the task, change course; if it asks you to stop, finish the step in hand cleanly and stop. Do not redo work that is already done.",
        message.trim()
    ))
}

/// What the engine knows about a background shell job when it ends.
///
/// `exit_code` is `None` when the
/// job was stopped or killed before it exited on its own; `output` is the tail
/// of its combined output, already trimmed to size by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundJob<'a> {
    pub id: &'a JobId,
    pub command: &'a str,
    pub exit_code: Option<i32>,
    pub output: &'a str,
    pub output_truncated: bool,
}

pub fn background_job_finished(job: &BackgroundJob<'_>) -> String {
    let status = match job.exit_code {
        Some(0) => "finished successfully (exit code 0)".to_owned(),
        Some(code) => format!("failed with exit code {code}"),
        None => "was stopped before it exited".to_owned(),
    };
    let output = job.output.trim_end();
    let output_part = if output.is_empty() {
        "It produced no output.".to_owned()
    } else if job.output_truncated {
        format!("The last part of its output:\n```\n{output}\n```")
    } else {
        format!("Its output:\n```\n{output}\n```")
    };
    reminder(&format!(
        "Background job {} {status}.\nCommand: {}\n{output_part}\nIf this affects what you are doing, act on it; otherwise carry on. Mention it to the user only if it matters to them.",
        job.id, job.command
    ))
}

pub const POLICY_DENIAL_PREFIX: &str = "Denied by policy";

pub fn tool_denied_by_policy(tool: &str, reason: &str) -> String {
    let reason = reason.trim();
    let reason = if reason.is_empty() {
        "no reason was given"
    } else {
        reason
    };
    format!(
        "{POLICY_DENIAL_PREFIX}: the {tool} call was blocked by your organisation's policy and did not run. Reason: {reason}\nDo not repeat this call, and do not try to reach the same result another way that gets around the policy. Tell the user in a sentence what was blocked and why, then continue with an approach the policy allows, or ask the user how to proceed if there is none."
    )
}

pub fn tool_rejected_by_user(tool: &str, feedback: Option<&str>) -> String {
    feedback.map(str::trim).filter(|text| !text.is_empty()).map_or_else(
        || {
            format!(
                "The user declined the {tool} call, so it did not run. Do not try it again. Stop and ask the user how they would like to proceed."
            )
        },
        |text| {
            format!(
                "The user declined the {tool} call, so it did not run. They said:\n\n{text}\n\nFollow their direction before going further."
            )
        },
    )
}

pub fn invalid_tool_input(tool: &str, error: &str) -> String {
    format!(
        "The {tool} call was not run because its input was invalid: {}\nCheck the tool's parameters, correct the input and call it again.",
        error.trim()
    )
}

pub fn unknown_tool(name: &str, available: &[&str]) -> String {
    if available.is_empty() {
        format!(
            "There is no tool named {name}, and no tools are available right now. Reply with text only."
        )
    } else {
        format!(
            "There is no tool named {name}. The tools available are: {}. Use one of these, with its exact name.",
            available.join(", ")
        )
    }
}

pub const MAX_STEPS: &str = r"<system-reminder>
This task has reached its step limit, and tools are now disabled until the user writes again. Reply with text only; any tool call will fail. This overrides every other instruction, including requests to keep editing.
In your reply:
- say that the step limit was reached;
- summarise what has been done;
- list what is still left to do;
- suggest how to continue.
</system-reminder>";
