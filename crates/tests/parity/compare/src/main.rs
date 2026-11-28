//! `parity-ledger`: gathers the ledger rows the golden runner wrote, adds the
//! harness-to-reference comparison when the reference runner's export is
//! given, and writes the parity ledger.
//!
//! ```text
//! parity-ledger --runs <dir> --out <dir> [--reference <dir> --scenarios <dir>]
//! ```
//!
//! `--runs` is the runner's output (`<scenario>/<protocol>/row.json` and
//! `trace.json`); `--reference` the reference runner's `--out`. The two
//! traces of every compared run are written to
//! `<out>/traces/<scenario>/<protocol>/{harness,reference}.json`. Exits 1
//! when any difference is unclassified or any run failed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use serde_json::{Value, json};
use systemprompt_parity_compare::ledger::HARNESS_VS_REFERENCE;
use systemprompt_parity_compare::reference::{self, Table};
use systemprompt_parity_compare::{
    Classifications, Ledger, LedgerRow, ScenarioScript, build, differences, digest,
};

struct Options {
    runs: PathBuf,
    out: PathBuf,
    reference: Option<PathBuf>,
    scenarios: Option<PathBuf>,
}

fn options() -> anyhow::Result<Options> {
    let mut args = std::env::args().skip(1);
    let (mut runs, mut out, mut reference, mut scenarios) = (None, None, None, None);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .map(PathBuf::from)
            .with_context(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--runs" => runs = Some(value),
            "--out" => out = Some(value),
            "--reference" => reference = Some(value),
            "--scenarios" => scenarios = Some(value),
            _ => anyhow::bail!("unknown argument {flag}"),
        }
    }
    Ok(Options {
        runs: runs.context("--runs is required")?,
        out: out.context("--out is required")?,
        reference,
        scenarios,
    })
}

fn subdirs(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    found.sort();
    Ok(found)
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The workspace the reference agent left, digested as the runner digests
/// the harness's: against the scenario's fixture.
fn reference_workspace(fixture: &Path, exported: &Path) -> anyhow::Result<Value> {
    let before = digest::files(&fixture.join("workspace"), &[])?;
    let after = digest::files(&exported.join("workspace"), &[".git"])?;
    Ok(json!({ "tree_digest": digest::tree(&after), "changed": digest::changed(&before, &after) }))
}

/// The harness's trace of a run and the reference agent's trace of the same
/// scenario and protocol.
fn traces(
    run: &Path,
    exported: &Path,
    fixture: &Path,
    protocol: &str,
) -> anyhow::Result<(Value, Value)> {
    let harness: Value = serde_json::from_str(&std::fs::read_to_string(run.join("trace.json"))?)?;
    anyhow::ensure!(!harness.is_null(), "the harness run left no trace");
    let table = Table::parse(&std::fs::read_to_string(
        fixture.join("../../../parity/reference/tools.json"),
    )?)?;
    let script = ScenarioScript::parse(&std::fs::read_to_string(run.join("turns.json"))?)?;
    let transcript = reference::transcript(exported, &table)?;
    let mut theirs = build::trace(
        &transcript,
        &script,
        protocol,
        reference_workspace(fixture, exported)?,
    );
    let mut harness = harness;
    if let Some(steps) = harness.get_mut("steps") {
        drop_unmapped_input(steps, &table);
    }
    if harness.pointer("/session/jobs").is_some() {
        theirs["session"]["jobs"] = reference_jobs(exported);
    }
    Ok((harness, theirs))
}

/// Removes from every `tool_call` step's input, subagent steps included, the
/// fields the reference table maps to nothing: the reference tool takes no
/// such field, so its side can never carry one.
fn drop_unmapped_input(steps: &mut Value, table: &Table) {
    for step in steps.as_array_mut().into_iter().flatten() {
        if step.get("kind").and_then(Value::as_str) == Some("tool_call") {
            let tool = step
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if let Some(input) = step.get_mut("input").and_then(Value::as_object_mut) {
                for field in table.dropped(&tool) {
                    input.remove(field);
                }
            }
        }
        if let Some(nested) = step.get_mut("steps") {
            drop_unmapped_input(nested, table);
        }
    }
}

/// The final state of each background job the reference agent listed at the
/// end of the run (`jobs.json`), in the harness trace's vocabulary.
fn reference_jobs(exported: &Path) -> Value {
    let Ok(text) = std::fs::read_to_string(exported.join("jobs.json")) else {
        return Value::Null;
    };
    let Ok(listed) = serde_json::from_str::<Value>(&text) else {
        return Value::Null;
    };
    listed
        .get("data")
        .and_then(Value::as_array)
        .map_or(Value::Null, |jobs| {
            jobs.iter()
                .map(|job| match job.get("status").and_then(Value::as_str) {
                    Some("running") => json!("running"),
                    Some("killed" | "aborted" | "cancelled" | "interrupted") => json!("killed"),
                    Some(other) => json!(other),
                    None => Value::Null,
                })
                .collect()
        })
}

fn keep(dir: &Path, harness: &Value, theirs: &Value) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        dir.join("harness.json"),
        serde_json::to_string_pretty(harness)?,
    )?;
    std::fs::write(
        dir.join("reference.json"),
        serde_json::to_string_pretty(theirs)?,
    )?;
    Ok(())
}

/// The harness-to-reference row of one run; none where the reference runner
/// exported no session for it.
fn reference_row(
    run: &Path,
    options: &Options,
    reference: &Path,
    scenarios: &Path,
    reviewed: &Classifications,
) -> Option<LedgerRow> {
    let protocol = name(run);
    let scenario = run.parent().map(name).unwrap_or_default();
    let exported = reference.join(&scenario).join(&protocol);
    if !exported.join("session.json").is_file() {
        return None;
    }
    let compared = traces(run, &exported, &scenarios.join(&scenario), &protocol).and_then(
        |(harness, theirs)| {
            let kept = options.out.join("traces").join(&scenario).join(&protocol);
            keep(&kept, &harness, &theirs)?;
            Ok(differences(&harness, &theirs, &[]))
        },
    );
    Some(match compared {
        Ok(found) => {
            LedgerRow::classify(&scenario, &protocol, HARNESS_VS_REFERENCE, found, reviewed)
        },
        Err(error) => LedgerRow::failed(
            &scenario,
            &protocol,
            HARNESS_VS_REFERENCE,
            format!("{error:#}"),
        ),
    })
}

fn gather(options: &Options) -> anyhow::Result<Ledger> {
    let reviewed = Classifications::reviewed()?;
    let mut ledger = Ledger::default();
    for scenario in subdirs(&options.runs)? {
        for run in subdirs(&scenario)? {
            let row: LedgerRow =
                serde_json::from_str(&std::fs::read_to_string(run.join("row.json"))?)?;
            ledger.rows.push(row);
            if let (Some(reference), Some(scenarios)) = (&options.reference, &options.scenarios)
                && let Some(row) = reference_row(&run, options, reference, scenarios, &reviewed)
            {
                ledger.rows.push(row);
            }
        }
    }
    Ok(ledger)
}

fn main() -> ExitCode {
    let ledger = options().and_then(|options| {
        let ledger = gather(&options)?;
        ledger.write(&options.out)?;
        Ok(ledger)
    });
    match ledger {
        Ok(ledger) => {
            println!("{}", ledger.markdown());
            let open = ledger
                .rows
                .iter()
                .any(|row| row.result == "error" || !row.unclassified().is_empty());
            if open {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        },
        Err(error) => {
            eprintln!("parity-ledger: {error:#}");
            ExitCode::FAILURE
        },
    }
}
