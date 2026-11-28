//! What a run is judged by: its normalised trace against `expected/trace.json`
//! and the harness-only assertions of `[harness]` and `[end]` (the stored
//! title, the verdict requests in order, the typed denials, the patched files,
//! session refreshes, the exit, paths that must not appear outside), each
//! difference classified into one ledger row.

use std::path::Path;

use serde_json::{Value, json};
use systemprompt_parity_compare::harness::{self, RecordedRequest};
use systemprompt_parity_compare::ledger::HARNESS_VS_EXPECTED;
use systemprompt_parity_compare::{
    Classifications, Difference, LedgerRow, ScenarioScript, build, differences, digest,
};
use systemprompt_scripted_instance::{RecordedBody, Route};

use crate::drive::RunOutput;
use crate::host::PROJECT_DIR;
use crate::scenario::Scenario;

#[derive(Debug)]
pub struct Outcome {
    pub trace: Value,
    pub row: LedgerRow,
}

pub(crate) fn body(record: &systemprompt_scripted_instance::RequestRecord) -> Value {
    match &record.body {
        RecordedBody::Json(json) => json.clone(),
        RecordedBody::Empty | RecordedBody::Bytes(_) => Value::Null,
    }
}

fn workspace(scenario: &Scenario, output: &RunOutput) -> anyhow::Result<Value> {
    let before = digest::files(&scenario.dir.join("workspace"), &[])?;
    let after = digest::files(&output.workspace, &[PROJECT_DIR, ".git"])?;
    Ok(json!({ "tree_digest": digest::tree(&after), "changed": digest::changed(&before, &after) }))
}

/// Normalises a run, compares it and classifies the differences.
pub fn judge(
    scenario: &Scenario,
    protocol: &str,
    output: &RunOutput,
    reviewed: &Classifications,
) -> anyhow::Result<Outcome> {
    let script = ScenarioScript::parse(&output.turns)?;
    let recorded: Vec<RecordedRequest> = output
        .requests
        .iter()
        .filter(|record| matches!(record.route, Route::Messages | Route::ChatCompletions))
        .map(|record| RecordedRequest {
            body: body(record),
            unscripted: record.unscripted,
        })
        .collect();
    let workspace_path = output.workspace.to_string_lossy().into_owned();
    let transcript = harness::transcript(
        &output.events,
        &recorded,
        output.marks.clone(),
        &workspace_path,
        &script,
    );
    let mut trace = build::trace(&transcript, &script, protocol, workspace(scenario, output)?);
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(
        scenario.dir.join("expected/trace.json"),
    )?)?;
    if expected.pointer("/session/jobs").is_some() {
        trace["session"]["jobs"] = crate::checks::jobs(&transcript, output.left_running);
    }
    let mut found = differences(&expected, &trace, &scenario.substitutions(protocol));
    let (wanted, observed) = crate::checks::harness_checks(scenario, output, &transcript);
    found.extend(differences(
        &json!({ "harness": wanted }),
        &json!({ "harness": observed }),
        &[],
    ));
    let row = LedgerRow::classify(&scenario.id, protocol, HARNESS_VS_EXPECTED, found, reviewed);
    Ok(Outcome { trace, row })
}

/// Writes what a run left and how it was judged under `<runs>/<id>/<protocol>`.
pub fn record(
    runs: &Path,
    scenario: &Scenario,
    protocol: &str,
    output: Option<&RunOutput>,
    outcome: &Outcome,
) -> anyhow::Result<()> {
    let dir = runs.join(&scenario.id).join(protocol);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("trace.json"),
        serde_json::to_string_pretty(&outcome.trace)?,
    )?;
    std::fs::write(
        dir.join("row.json"),
        serde_json::to_string_pretty(&outcome.row)?,
    )?;
    if let Some(output) = output {
        let events: Vec<String> = output.events.iter().map(Value::to_string).collect();
        std::fs::write(dir.join("events.jsonl"), events.join("\n"))?;
        std::fs::write(dir.join("turns.json"), &output.turns)?;
        let requests: Vec<Value> = output
            .requests
            .iter()
            .map(|record| json!({ "route": format!("{:?}", record.route), "path": record.path, "unscripted": record.unscripted, "body": body(record) }))
            .collect();
        std::fs::write(
            dir.join("requests.json"),
            serde_json::to_string_pretty(&requests)?,
        )?;
    }
    Ok(())
}

/// A ledger row for a run that could not be judged.
pub fn failed(scenario: &Scenario, protocol: &str, error: &anyhow::Error) -> Outcome {
    let row = LedgerRow::failed(
        &scenario.id,
        protocol,
        HARNESS_VS_EXPECTED,
        format!("{error:#}"),
    );
    Outcome {
        trace: Value::Null,
        row,
    }
}

pub fn unclassified(row: &LedgerRow) -> Vec<&Difference> {
    row.unclassified()
        .into_iter()
        .map(|entry| &entry.difference)
        .collect()
}
