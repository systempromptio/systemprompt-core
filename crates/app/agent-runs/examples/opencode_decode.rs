//! Replays a captured `opencode run --format json` stream through the core
//! decoder and outcome classifier.
//!
//! Reads JSONL on stdin and writes one JSON document on stdout:
//! `{"events": [RunEventPayload...], "outcome": RunOutcome}`. The exit is
//! taken as code 0, neither cancelled nor timed out, so the outcome reflects
//! the events alone. Used to check a new `OpenCode` release against the
//! decoder without building the test workspace.
//!
//! ```text
//! cargo run -p systemprompt-agent-runs --example opencode_decode < run.jsonl
//! ```

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use systemprompt_agent_runs::host::opencode::OpenCodeCli;
use systemprompt_agent_runs::{
    ExitInfo, HeadlessHost, HostKind, PermissionMode, RunRequest, TrafficClass,
};

const fn request() -> RunRequest {
    RunRequest {
        host: HostKind::OpenCodeCli,
        prompt: String::new(),
        system_prompt_append: None,
        persona: None,
        model: None,
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: PermissionMode::Default,
        permission_prompts_none: true,
        workspace: PathBuf::new(),
        add_dirs: Vec::new(),
        mcp_servers: BTreeMap::new(),
        output_schema: None,
        max_turns: None,
        max_budget_usd: None,
        wall_clock: Duration::from_secs(0),
        resume_session_id: None,
        attempt: 1,
        attribution_scopes: Vec::new(),
        plugin_id: None,
        plugin_dirs: Vec::new(),
        harness: None,
        traffic: TrafficClass::Production,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = OpenCodeCli::default();
    let mut events = Vec::new();
    for line in std::io::stdin().lock().lines() {
        events.extend(host.parse_line(&line?).into_iter().flat_map(|e| e.payloads));
    }
    let exit = ExitInfo {
        code: Some(0),
        timed_out: false,
        cancelled: false,
        duration_ms: 0,
    };
    let outcome = host.outcome(exit, &events, &request());
    let doc = serde_json::json!({ "events": events, "outcome": outcome });
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &doc)?;
    writeln!(out)?;
    Ok(())
}
