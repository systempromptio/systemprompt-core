use systemprompt_core_logging::{
    AiRequestInfo, CliService, ConversationMessage, ExecutionStep, TaskInfo,
};
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Tabled)]
pub struct TaskInfoRow {
    #[tabled(rename = "Task ID")]
    pub task_id: String,
    #[tabled(rename = "Agent")]
    pub agent_name: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Started")]
    pub started_at: String,
    #[tabled(rename = "Duration")]
    pub duration: String,
}

#[derive(Tabled)]
pub struct StepRow {
    #[tabled(rename = "#")]
    pub step_number: i32,
    #[tabled(rename = "Type")]
    pub step_type: String,
    #[tabled(rename = "Title")]
    pub title: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Duration")]
    pub duration: String,
}

#[derive(Tabled)]
pub struct AiRequestRow {
    #[tabled(rename = "Model")]
    pub model: String,
    #[tabled(rename = "Max")]
    pub max_tokens: String,
    #[tabled(rename = "Tokens")]
    pub tokens: String,
    #[tabled(rename = "Cost")]
    pub cost: String,
    #[tabled(rename = "Latency")]
    pub latency: String,
}

#[derive(Tabled)]
pub struct ToolCallRow {
    #[tabled(rename = "Tool")]
    pub tool_name: String,
    #[tabled(rename = "Server")]
    pub server: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Duration")]
    pub duration: String,
}

#[derive(Tabled)]
pub struct ArtifactRow {
    #[tabled(rename = "ID")]
    pub artifact_id: String,
    #[tabled(rename = "Type")]
    pub artifact_type: String,
    #[tabled(rename = "Name")]
    pub name: String,
    #[tabled(rename = "Source")]
    pub source: String,
    #[tabled(rename = "Tool")]
    pub tool_name: String,
}

pub fn truncate(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ").replace('\r', "");
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s
    }
}

pub fn print_section(title: &str) {
    CliService::section(title);
}

pub fn print_content_block(content: &str) {
    for line in content.lines() {
        CliService::info(&format!("  {line}"));
    }
}

pub fn print_task_info(task_info: &TaskInfo) {
    let rows = vec![TaskInfoRow {
        task_id: task_info.task_id[..8].to_string(),
        agent_name: task_info
            .agent_name
            .clone()
            .unwrap_or_else(|| "-".to_string()),
        status: task_info.status.clone(),
        started_at: task_info
            .started_at
            .map_or_else(|| "-".to_string(), |t| t.format("%H:%M:%S").to_string()),
        duration: task_info
            .execution_time_ms
            .map_or_else(|| "-".to_string(), |ms| format!("{}ms", ms)),
    }];

    print_section("TASK");
    let table = Table::new(rows).with(Style::rounded()).to_string();
    CliService::info(&table);

    // Display error message if task failed
    if task_info.status == "failed" {
        if let Some(ref error) = task_info.error_message {
            if !error.is_empty() {
                CliService::error("Error:");
                print_content_block(error);
            }
        }
    }
}

pub fn print_user_input(input: Option<&String>) {
    if let Some(text) = input {
        print_section("USER INPUT");
        CliService::info(&format!("  {text}"));
    }
}

pub fn print_agent_response(response: Option<&String>) {
    if let Some(text) = response {
        print_section("AGENT RESPONSE");
        print_content_block(text);
    }
}

pub fn print_execution_steps(steps: &[ExecutionStep]) {
    if steps.is_empty() {
        return;
    }

    let step_rows: Vec<StepRow> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| StepRow {
            step_number: (i + 1) as i32,
            step_type: s.step_type.clone().unwrap_or_else(|| "unknown".to_string()),
            title: truncate(&s.title.clone().unwrap_or_default(), 40),
            status: s.status.clone(),
            duration: s
                .duration_ms
                .map_or_else(|| "-".to_string(), |ms| format!("{}ms", ms)),
        })
        .collect();

    print_section("EXECUTION STEPS");
    let table = Table::new(step_rows).with(Style::rounded()).to_string();
    CliService::info(&table);

    for step in steps {
        if step.status == "failed" {
            if let Some(ref error) = step.error_message {
                if !error.is_empty() {
                    let step_type = step.step_type.clone().unwrap_or_else(|| "step".to_string());
                    CliService::error(&format!("  {step_type} failed:"));
                    print_content_block(error);
                }
            }
        }
    }
}

pub fn print_ai_requests(requests: &[AiRequestInfo]) -> Vec<String> {
    if requests.is_empty() {
        return vec![];
    }

    let request_ids: Vec<String> = requests.iter().map(|r| r.id.clone()).collect();

    let ai_rows: Vec<AiRequestRow> = requests
        .iter()
        .map(|r| AiRequestRow {
            model: format!("{}/{}", r.provider, r.model),
            max_tokens: r
                .max_tokens
                .map_or_else(|| "-".to_string(), |t| t.to_string()),
            tokens: format!(
                "{} (in:{}, out:{})",
                r.input_tokens.unwrap_or(0) + r.output_tokens.unwrap_or(0),
                r.input_tokens.unwrap_or(0),
                r.output_tokens.unwrap_or(0)
            ),
            cost: format!("${:.4}", f64::from(r.cost_cents) / 1_000_000.0),
            latency: r
                .latency_ms
                .map_or_else(|| "-".to_string(), |ms| format!("{}ms", ms)),
        })
        .collect();

    print_section("AI REQUESTS");
    let table = Table::new(ai_rows).with(Style::rounded()).to_string();
    CliService::info(&table);

    request_ids
}

#[allow(dead_code)]
pub fn print_system_prompt(prompt: Option<&String>) {
    if let Some(content) = prompt {
        print_section("SYSTEM PROMPT");
        print_content_block(content);
    }
}

#[allow(dead_code)]
pub fn print_conversation_history(messages_by_request: &[(usize, Vec<ConversationMessage>)]) {
    if messages_by_request.is_empty() {
        return;
    }

    print_section("CONVERSATION HISTORY");

    for (req_idx, messages) in messages_by_request {
        if messages.is_empty() {
            continue;
        }

        CliService::info(&format!("── Request {} ──", req_idx + 1));

        for msg in messages {
            let label = match msg.role.as_str() {
                "system" => "SYSTEM",
                "user" => "USER",
                "assistant" => "ASSISTANT",
                _ => &msg.role,
            };

            CliService::info(&format!("[{label}] #{}", msg.sequence_number));

            if msg.role == "system" && msg.content.len() > 500 {
                CliService::info(&format!("  [System prompt: {} chars]", msg.content.len()));
            } else {
                print_content_block(&msg.content);
            }
        }
    }
}
