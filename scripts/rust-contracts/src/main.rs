//! Rust source contracts for explicit result handling and closed guards.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

fn scan(path: &Path, mode: &str, count: &mut usize) -> Result<bool, Box<dyn std::error::Error>> {
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlink in source scan: {}", path.display()).into());
    }
    if metadata.is_dir() {
        if path
            .file_name()
            .is_some_and(|name| name == "tests" || name == "target")
        {
            return Ok(false);
        }
        let mut failed = false;
        for entry in std::fs::read_dir(path)? {
            failed |= scan(&entry?.path(), mode, count)?;
        }
        return Ok(failed);
    }
    if path.extension().is_none_or(|extension| extension != "rs")
        || path.file_name().is_some_and(|name| name == "build.rs")
    {
        return Ok(false);
    }
    *count += 1;
    let source = std::fs::read_to_string(path)?;
    let findings = rust_contracts::inspect(&source, mode)
        .map_err(|e| format!("{}: cannot parse Rust: {e}", path.display()))?;
    for finding in &findings {
        eprintln!("{}:{}: {}", path.display(), finding.line, finding.rule);
    }
    Ok(!findings.is_empty())
}

fn main() -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .ok_or("expected discarded or fail-open and source roots")?;
    if !matches!(mode.as_str(), "discarded" | "fail-open") {
        return Err("unknown scan mode".into());
    }
    let mut count = 0;
    let mut failed = false;
    for root in args {
        failed |= scan(Path::new(&root), &mode, &mut count)?;
    }
    if count == 0 {
        return Err("no Rust source files scanned".into());
    }
    if failed {
        return Ok(std::process::ExitCode::FAILURE);
    }
    println!("{mode}: checked {count} Rust files");
    Ok(std::process::ExitCode::SUCCESS)
}
