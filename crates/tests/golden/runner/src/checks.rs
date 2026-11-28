//! The harness-only assertions of `[harness]` and `[end]`, read from the
//! store, the scripted instance's request log and the workspace: the stored
//! title, the verdict requests in order, the typed denials, the patched files,
//! session refreshes, the exit, paths that must not appear outside, and the
//! final state of background jobs.

use serde_json::{Value, json};
use systemprompt_harness::command::ExitStatus;
use systemprompt_parity_compare::{CallStatus, Message, ScenarioScript, Transcript};
use systemprompt_scripted_instance::Route;

use crate::drive::RunOutput;
use crate::outcome::body;
use crate::scenario::Scenario;

fn exit_name(status: ExitStatus) -> &'static str {
    match status {
        ExitStatus::Completed => "ok",
        ExitStatus::Aborted => "aborted",
        ExitStatus::Failed => "failed",
        _ => "other",
    }
}

fn denials(transcript: &Transcript) -> Value {
    let mut found = serde_json::Map::new();
    for log in transcript.sessions.values() {
        for message in &log.messages {
            let Message::Assistant(reply) = message else {
                continue;
            };
            for call in &reply.calls {
                let by = match call.status {
                    CallStatus::DeniedByPolicy => "policy",
                    CallStatus::DeniedByRule => "rule",
                    CallStatus::Rejected => "rejected",
                    CallStatus::Aborted => "aborted",
                    _ => continue,
                };
                found.insert(call.id.clone(), json!(by));
            }
        }
    }
    Value::Object(found)
}

fn patch_files(
    events: &[Value],
    workspace: &str,
    hidden: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    let prefix = format!("{}/", workspace.trim_end_matches('/'));
    let mut files: Vec<String> = events
        .iter()
        .filter(|event| event.pointer("/part/body/type").and_then(Value::as_str) == Some("patch"))
        .filter(|event| {
            let message = event
                .pointer("/part/message_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            !hidden.contains(message)
        })
        .flat_map(|event| {
            event
                .pointer("/part/body/files")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|file| {
            file.as_str()
                .map(|file| file.strip_prefix(&prefix).unwrap_or(file).to_owned())
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

/// The observed side of every harness-only assertion the scenario makes,
/// beside the expected side, as one object each.
pub(crate) fn harness_checks(
    scenario: &Scenario,
    output: &RunOutput,
    transcript: &Transcript,
) -> (Value, Value) {
    let workspace = output.workspace.to_string_lossy();
    let during: Vec<_> = output
        .requests
        .iter()
        .skip(output.requests_before_run)
        .collect();
    let issued: Vec<String> = ScenarioScript::parse(&output.turns)
        .map(|script| {
            script
                .turns
                .into_iter()
                .flat_map(|turn| turn.calls)
                .collect()
        })
        .unwrap_or_default();
    let mut verdicts: Vec<(usize, Value)> = during
        .iter()
        .filter(|record| record.route == Route::Verdict)
        .map(|record| {
            let body = body(record);
            let call = body
                .get("tool_use_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let order = issued
                .iter()
                .position(|id| id == call)
                .unwrap_or(usize::MAX);
            (order, body.get("tool_name").cloned().unwrap_or(Value::Null))
        })
        .collect();
    // Why: the calls of one response run concurrently, so their verdict
    // requests reach the instance in either order; they are compared in the
    // order the model issued the calls (the corpus's rule for the trace).
    verdicts.sort_by_key(|(order, _)| *order);
    let verdicts: Vec<Value> = verdicts.into_iter().map(|(_, tool)| tool).collect();
    let first_model = during
        .iter()
        .position(|record| matches!(record.route, Route::Messages | Route::ChatCompletions));
    let refreshes = first_model.map_or(0, |first| {
        during[first..]
            .iter()
            .filter(|record| matches!(record.route, Route::PatSession | Route::CodeExchange))
            .count()
    });
    let title = transcript
        .session(&transcript.root)
        .and_then(|log| log.title.clone());
    let mut expected =
        json!({ "exit": scenario.exit, "unconsumed_turns": 0, "unscripted_requests": 0 });
    let mut actual = json!({
        "exit": exit_name(output.report.status),
        "unconsumed_turns": output.unconsumed_turns,
        "unscripted_requests": during.iter().filter(|record| record.unscripted).count(),
    });
    let wanted = &scenario.harness;
    if let Some(value) = wanted.get("title") {
        expected["title"] = json!(value.as_str());
        actual["title"] = json!(title);
    }
    if let Some(value) = wanted.get("verdicts") {
        expected["verdicts"] = serde_json::to_value(value).unwrap_or(Value::Null);
        actual["verdicts"] = json!(verdicts);
    }
    if let Some(value) = wanted.get("session_refreshes") {
        expected["session_refreshes"] = serde_json::to_value(value).unwrap_or(Value::Null);
        actual["session_refreshes"] = json!(refreshes);
    }
    if let Some(value) = wanted.get("patch_files") {
        expected["patch_files"] = serde_json::to_value(value).unwrap_or(Value::Null);
        actual["patch_files"] = json!(patch_files(&output.events, &workspace, &transcript.hidden));
    }
    if let Some(value) = wanted.get("denial").and_then(toml::Value::as_array) {
        let observed = denials(transcript);
        let mut want = serde_json::Map::new();
        let mut got = serde_json::Map::new();
        for denial in value.iter().filter_map(toml::Value::as_table) {
            let call = denial
                .get("call")
                .and_then(toml::Value::as_str)
                .unwrap_or_default();
            want.insert(
                call.to_owned(),
                json!(denial.get("by").and_then(toml::Value::as_str)),
            );
            got.insert(
                call.to_owned(),
                observed.get(call).cloned().unwrap_or(Value::Null),
            );
        }
        expected["denials"] = Value::Object(want);
        actual["denials"] = Value::Object(got);
    }
    for path in &scenario.absent_outside {
        expected["absent_outside"][path] = json!(false);
        actual["absent_outside"][path] = json!(output.workspace.join(path).exists());
    }
    (expected, actual)
}

/// The final state of each background job: the harness's stream does not
/// carry its job table, so a job counts as killed when no process is left
/// running in the workspace.
pub(crate) fn jobs(transcript: &Transcript, left_running: usize) -> Value {
    let started = transcript
        .sessions
        .values()
        .flat_map(|log| &log.messages)
        .filter_map(|message| match message {
            Message::Assistant(reply) => Some(reply),
            Message::User { .. } => None,
        })
        .flat_map(|reply| &reply.calls)
        .filter(|call| call.input.get("background").and_then(Value::as_bool) == Some(true))
        .count();
    let state = if left_running == 0 {
        "killed"
    } else {
        "running"
    };
    json!(vec![state; started])
}
