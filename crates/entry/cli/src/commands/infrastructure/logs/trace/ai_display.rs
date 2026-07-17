//! AI-request rendering for trace views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_logging::{AiRequestInfo, CliService, ExecutionStep, TaskInfo};

use crate::presentation::tables::{ai_requests_table, execution_steps_table, task_info_table};

pub(super) use crate::presentation::tables::truncate_cell as truncate;

pub(super) fn print_section(title: &str) {
    CliService::section(title);
}

pub(super) fn print_content_block(content: &str) {
    for line in content.lines() {
        CliService::info(&format!("  {line}"));
    }
}

pub(super) fn print_task_info(task_info: &TaskInfo) {
    print_section("TASK");
    CliService::info(&task_info_table(task_info));

    if task_info.status == "failed"
        && let Some(ref error) = task_info.error_message
        && !error.is_empty()
    {
        CliService::error("Error:");
        print_content_block(error);
    }
}

pub(super) fn print_user_input(input: Option<&String>) {
    if let Some(text) = input {
        print_section("USER INPUT");
        CliService::info(&format!("  {text}"));
    }
}

pub(super) fn print_agent_response(response: Option<&String>) {
    if let Some(text) = response {
        print_section("AGENT RESPONSE");
        print_content_block(text);
    }
}

pub(super) fn print_execution_steps(steps: &[ExecutionStep]) {
    if steps.is_empty() {
        return;
    }

    print_section("EXECUTION STEPS");
    CliService::info(&execution_steps_table(steps));

    for step in steps {
        if step.status == "failed"
            && let Some(ref error) = step.error_message
            && !error.is_empty()
        {
            let step_type = step.step_type.clone().unwrap_or_else(|| "step".to_owned());
            CliService::error(&format!("  {step_type} failed:"));
            print_content_block(error);
        }
    }
}

pub(super) fn print_ai_requests(requests: &[AiRequestInfo]) -> Vec<String> {
    if requests.is_empty() {
        return vec![];
    }

    let request_ids: Vec<String> = requests.iter().map(|r| r.id.to_string()).collect();

    print_section("AI REQUESTS");
    CliService::info(&ai_requests_table(requests));

    request_ids
}
