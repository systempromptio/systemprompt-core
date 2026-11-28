//! The comparator's parts against small fixed inputs: the digests of the
//! corpus's rules, the trace differences, the ledger's classification, and the
//! reference adapter over a hand-written export.

use std::path::Path;

use serde_json::json;
use systemprompt_parity_compare::ledger::HARNESS_VS_EXPECTED;
use systemprompt_parity_compare::reference::{self, Table};
use systemprompt_parity_compare::{
    Classifications, Ledger, LedgerRow, ScenarioScript, build, differences, digest,
};

#[test]
fn a_text_digest_ignores_line_endings_trailing_blanks_and_final_newlines() {
    assert_eq!(digest::text("a  \r\nb\t\n\n"), digest::text("a\nb"));
    assert_ne!(digest::text("a\nb"), digest::text("a\n b"));
}

#[test]
fn placeholders_are_substituted_and_a_null_digest_is_not_compared() {
    let expected = json!({ "steps": [
        { "kind": "model_request", "protocol": "{{protocol}}", "tool_results": [] },
        { "kind": "tool_result", "digest": null, "status": "ok" },
    ] });
    let actual = json!({ "steps": [
        { "kind": "model_request", "protocol": "chat", "tool_results": [] },
        { "kind": "tool_result", "digest": "sha256:00", "status": "error" },
    ] });
    let found = differences(
        &expected,
        &actual,
        &[("{{protocol}}".to_owned(), "chat".to_owned())],
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].path, "steps[1].status");
}

#[test]
fn a_difference_the_reviewed_file_does_not_name_is_unclassified() {
    let reviewed: Classifications = toml::from_str(
        r#"
        [[entry]]
        scenario = "01-x"
        path = "steps[1].*"
        class = "intended"
        reason = "the harness offers no such tool"
        "#,
    )
    .unwrap();
    reviewed.check().unwrap();
    let found = differences(
        &json!({ "a": 1, "steps": [0, { "b": 1 }] }),
        &json!({ "a": 2, "steps": [0, { "b": 2 }] }),
        &[],
    );
    let row = LedgerRow::classify("01-x", "messages", HARNESS_VS_EXPECTED, found, &reviewed);
    let classes: Vec<&str> = row
        .entries
        .iter()
        .map(|entry| entry.class.as_str())
        .collect();
    assert_eq!(classes, ["unclassified", "intended"]);
    let unowned: Classifications = toml::from_str(
        "[[entry]]\nscenario = \"x\"\npath = \"a\"\nclass = \"gap\"\nreason = \"r\"\n",
    )
    .unwrap();
    assert!(unowned.check().is_err(), "a gap needs an owner stage");
}

#[test]
fn a_row_takes_its_worst_class_and_the_ledger_totals_each_comparison() {
    let reviewed: Classifications = toml::from_str(
        r#"
        [[entry]]
        scenario = "01-x"
        path = "a"
        class = "intended"
        reason = "by design"

        [[entry]]
        scenario = "01-x"
        path = "b"
        class = "defect"
        reason = "the harness is wrong"
        "#,
    )
    .unwrap();
    let found = differences(&json!({ "a": 1, "b": 1 }), &json!({ "a": 2, "b": 2 }), &[]);
    let differs = LedgerRow::classify("01-x", "messages", HARNESS_VS_EXPECTED, found, &reviewed);
    assert_eq!(differs.verdict(), "defect");
    let same = LedgerRow::classify("01-x", "chat", HARNESS_VS_EXPECTED, Vec::new(), &reviewed);
    assert_eq!(same.verdict(), "same");
    let failed = LedgerRow::failed("02-y", "chat", HARNESS_VS_EXPECTED, "no trace".to_owned());
    assert_eq!(failed.verdict(), "error");
    let ledger = Ledger {
        rows: vec![differs, same, failed],
    };
    let written: serde_json::Value = serde_json::from_str(&ledger.json().unwrap()).unwrap();
    let totals = &written["totals"][HARNESS_VS_EXPECTED];
    assert_eq!(
        totals["runs"],
        json!({ "defect": 1, "error": 1, "same": 1 })
    );
    assert_eq!(totals["differences"], json!({ "defect": 1, "intended": 1 }));
    assert_eq!(written["rows"][0]["verdict"], json!("defect"));
    let read: Ledger = serde_json::from_str(&ledger.json().unwrap()).unwrap();
    assert_eq!(read.rows, ledger.rows);
}

#[test]
fn the_reviewed_file_is_well_formed() {
    Classifications::reviewed().unwrap();
}

#[test]
fn a_reference_export_reduces_to_the_trace_with_logical_names() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path();
    std::fs::write(
        run.join("session.json"),
        json!({
            "info": { "id": "root" },
            "messages": [
                { "type": "user", "id": "u1", "text": "Read a." },
                { "type": "assistant", "id": "a1", "finish": "tool-calls", "content": [
                    { "type": "tool", "id": "t1", "name": "read", "state": {
                        "status": "completed", "input": { "path": run.join("workspace/a.txt") }, "content": "one" } },
                ] },
                { "type": "assistant", "id": "a2", "finish": "stop", "content": [{ "type": "text", "text": "Done." }] },
            ],
        })
        .to_string(),
    )
    .unwrap();
    let requests = [
        json!({ "title_call": true, "body": { "messages": [] } }),
        json!({ "status": 200, "body": { "tools": [{ "name": "read" }], "messages": [] } }),
        json!({ "status": 200, "body": { "tools": [{ "name": "read" }], "messages": [
            { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "t1" }] }] } }),
    ];
    let lines: Vec<String> = requests.iter().map(ToString::to_string).collect();
    std::fs::write(run.join("requests.jsonl"), lines.join("\n")).unwrap();
    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/tools.json");
    let table = Table::parse(&std::fs::read_to_string(tools).unwrap()).unwrap();
    let transcript = reference::transcript(run, &table).unwrap();
    let script = ScenarioScript::parse(
        &json!({ "turns": [
            { "respond": [{ "tool_call": { "id": "t1", "name": "read", "input": { "path": "a.txt" } } }] },
            { "respond": [{ "text": "Done." }] },
        ] })
        .to_string(),
    )
    .unwrap();
    let trace = build::trace(&transcript, &script, "messages", json!({}));
    let kinds: Vec<&str> = trace["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "model_request",
            "tool_call",
            "permission",
            "tool_result",
            "model_request",
            "assistant_text",
            "finish"
        ]
    );
    assert_eq!(trace["steps"][1]["input"], json!({ "path": "a.txt" }));
    assert_eq!(trace["steps"][3]["digest"], json!(digest::text("one")));
    assert_eq!(trace["steps"][4]["tool_results"], json!(["t1"]));
    assert_eq!(trace["session"]["messages"], json!(4));
}

#[test]
fn a_reference_export_keeps_errors_truncation_asks_compaction_and_todos() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path();
    std::fs::write(
        run.join("session.json"),
        json!({
            "info": { "id": "root" },
            "messages": [
                { "type": "user", "id": "u1", "text": "Go." },
                { "type": "assistant", "id": "a1", "finish": "tool-calls", "content": [
                    { "type": "tool", "id": "t1", "name": "shell", "state": {
                        "status": "error", "input": { "command": "echo x" },
                        "error": { "type": "permission.rejected", "message": "Not now." } } },
                    { "type": "tool", "id": "t2", "name": "write", "state": {
                        "status": "error", "input": { "path": "s/a.txt", "content": "x" },
                        "error": { "type": "permission.rejected", "message": "Permission denied: edit" } } },
                    { "type": "tool", "id": "t3", "name": "read", "state": {
                        "status": "completed", "input": { "path": "a.txt" },
                        "content": [{ "type": "text", "text": "cut" }], "metadata": { "truncated": true } } },
                    { "type": "tool", "id": "t4", "name": "todowrite", "state": {
                        "status": "completed", "input": { "todos": [{ "content": "one", "status": "pending" }] },
                        "content": "ok" } },
                ] },
                { "type": "user", "id": "u2", "text": "More." },
                { "type": "compaction", "id": "c1", "reason": "auto", "summary": "## Goal" },
                { "type": "assistant", "id": "a2", "finish": "stop", "content": [{ "type": "text", "text": "Done." }] },
            ],
        })
        .to_string(),
    )
    .unwrap();
    let tools_body = json!([{ "name": "read" }]);
    let requests = [
        json!({ "status": 200, "body": { "tools": tools_body, "messages": [] } }),
        json!({ "status": 200, "body": { "tools": tools_body, "messages": [] } }),
        json!({ "status": 200, "body": { "tools": tools_body, "messages": [] } }),
    ];
    let lines: Vec<String> = requests.iter().map(ToString::to_string).collect();
    std::fs::write(run.join("requests.jsonl"), lines.join("\n")).unwrap();
    std::fs::write(
        run.join("run.json"),
        json!({
            "actions": [{ "do": "prompt", "message": 2, "on": "idle", "index": 1 }],
            "asks": [{ "index": 1, "request": { "source": { "id": "t1" } },
                       "reply": { "decision": "reject", "message": "Not now." } }],
        })
        .to_string(),
    )
    .unwrap();
    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/tools.json");
    let table = Table::parse(&std::fs::read_to_string(tools).unwrap()).unwrap();
    assert_eq!(table.dropped("shell"), ["description".to_owned()]);
    let transcript = reference::transcript(run, &table).unwrap();
    let script = ScenarioScript::parse(
        &json!({ "turns": [
            { "respond": [
                { "tool_call": { "id": "t1" } }, { "tool_call": { "id": "t2" } },
                { "tool_call": { "id": "t3" } }, { "tool_call": { "id": "t4" } },
            ] },
            { "respond": [{ "text": "## Goal" }] },
            { "respond": [{ "text": "Done." }] },
        ] })
        .to_string(),
    )
    .unwrap();
    let trace = build::trace(&transcript, &script, "messages", json!({}));
    let steps = trace["steps"].as_array().unwrap();
    let find = |call: &str, kind: &str| {
        steps
            .iter()
            .find(|step| step["call"] == json!(call) && step["kind"] == json!(kind))
            .unwrap()
            .clone()
    };
    assert_eq!(find("t1", "permission")["outcome"], json!("rejected"));
    assert_eq!(find("t1", "tool_result")["status"], json!("rejected"));
    assert_eq!(
        find("t1", "tool_result")["digest"],
        json!(digest::text("Not now."))
    );
    assert_eq!(find("t2", "permission")["outcome"], json!("denied_by_rule"));
    assert_eq!(find("t2", "tool_result")["status"], json!("denied"));
    assert_eq!(find("t3", "tool_result")["truncated"], json!(true));
    let kinds: Vec<&str> = steps
        .iter()
        .map(|step| step["kind"].as_str().unwrap())
        .collect();
    let prompt = kinds
        .iter()
        .position(|kind| *kind == "user_message")
        .unwrap();
    assert_eq!(kinds[prompt + 1], "model_request");
    assert_eq!(steps[prompt + 1]["purpose"], json!("compaction"));
    assert_eq!(kinds[prompt + 2], "compaction");
    assert_eq!(
        trace["session"]["todos"],
        json!([{ "content": "one", "status": "pending" }])
    );
}
