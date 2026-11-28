//! `parity-ledger`: gathers the ledger rows the golden runner wrote, adds the
//! harness-to-reference comparison when the reference runner's export is
//! given, and writes the parity ledger.
//!
//! `parity-ledger --runs <dir> --out <dir> [--reference <dir> --scenarios
//! <dir>]`
//!
//! `--runs` is the runner's output (`<scenario>/<protocol>/row.json` and
//! `trace.json`); `--reference` the reference runner's `--out`. Exits 1
//! when any difference is unclassified or any run failed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use serde_json::Value;
use systemprompt_parity_compare::ledger::HARNESS_VS_REFERENCE;
use systemprompt_parity_compare::reference::{self, Table};
use systemprompt_parity_compare::{
    Classifications, Ledger, LedgerRow, ScenarioScript, build, differences,
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

fn reference_row(
    run: &Path,
    reference: &Path,
    scenarios: &Path,
    reviewed: &Classifications,
) -> anyhow::Result<Option<LedgerRow>> {
    let protocol = name(run);
    let scenario = run.parent().map(name).unwrap_or_default();
    let exported = reference.join(&scenario).join(&protocol);
    if !exported.join("session.json").is_file() {
        return Ok(None);
    }
    let harness: Value = serde_json::from_str(&std::fs::read_to_string(run.join("trace.json"))?)?;
    let table = Table::parse(&std::fs::read_to_string(
        scenarios.join("../../parity/reference/tools.json"),
    )?)?;
    let script = ScenarioScript::parse(&std::fs::read_to_string(run.join("turns.json"))?)?;
    let transcript = reference::transcript(&exported, &table)?;
    let trace = build::trace(
        &transcript,
        &script,
        &protocol,
        harness["workspace"].clone(),
    );
    let found = differences(&harness, &trace, &[]);
    Ok(Some(LedgerRow::classify(
        &scenario,
        &protocol,
        HARNESS_VS_REFERENCE,
        found,
        reviewed,
    )))
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
                && let Some(row) = reference_row(&run, reference, scenarios, &reviewed)?
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
