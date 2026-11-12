//! Rust source contracts for explicit result handling and closed guards.
//!
//! `rust-contracts [--cache-dir DIR] MODE ROOT...` reports MODE's findings.
//! With `--cache-dir`, one walk parses every file once and evaluates all
//! modes; each mode's output and exit status is stored under a key derived
//! from this executable, the roots, and every scanned path and its contents,
//! so a later invocation over byte-identical inputs replays its mode's result
//! and any changed input recomputes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

type BoxError = Box<dyn std::error::Error>;

const MODES: [&str; 4] = [
    "discarded",
    "fail-open",
    "tracing-messages",
    "swallowed-errors",
];

struct Source {
    path: PathBuf,
    text: String,
}

struct Outcome {
    stderr: String,
    stdout: String,
    failed: bool,
}

fn walk(path: &Path, sources: &mut Vec<Source>) -> Result<(), BoxError> {
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlink in source scan: {}", path.display()).into());
    }
    if metadata.is_dir() {
        if path
            .file_name()
            .is_some_and(|name| name == "tests" || name == "target")
        {
            return Ok(());
        }
        for entry in std::fs::read_dir(path)? {
            walk(&entry?.path(), sources)?;
        }
        return Ok(());
    }
    if path.extension().is_none_or(|extension| extension != "rs")
        || path.file_name().is_some_and(|name| name == "build.rs")
    {
        return Ok(());
    }
    let text = std::fs::read_to_string(path)?;
    sources.push(Source {
        path: path.to_path_buf(),
        text,
    });
    Ok(())
}

fn collect(roots: &[String]) -> (Vec<Source>, Option<BoxError>) {
    let mut sources = Vec::new();
    for root in roots {
        if let Err(walk_error) = walk(Path::new(root), &mut sources) {
            return (sources, Some(walk_error));
        }
    }
    (sources, None)
}

struct Evaluation {
    outcomes: Vec<Outcome>,
    error: Option<BoxError>,
}

fn evaluate(sources: &[Source], modes: &[&str]) -> Evaluation {
    let mut outcomes: Vec<Outcome> = modes
        .iter()
        .map(|_| Outcome {
            stderr: String::new(),
            stdout: String::new(),
            failed: false,
        })
        .collect();
    for source in sources {
        let per_mode = match rust_contracts::inspect_modes(&source.text, modes) {
            Ok(per_mode) => per_mode,
            Err(e) => {
                let error = format!("{}: cannot parse Rust: {e}", source.path.display());
                return Evaluation {
                    outcomes,
                    error: Some(error.into()),
                };
            },
        };
        for (outcome, findings) in outcomes.iter_mut().zip(per_mode) {
            for finding in &findings {
                outcome.stderr.push_str(&format!(
                    "{}:{}: {}\n",
                    source.path.display(),
                    finding.line,
                    finding.rule
                ));
            }
            outcome.failed |= !findings.is_empty();
        }
    }
    for (outcome, mode) in outcomes.iter_mut().zip(modes) {
        if !outcome.failed {
            outcome.stdout = format!("{mode}: checked {} Rust files\n", sources.len());
        }
    }
    Evaluation {
        outcomes,
        error: None,
    }
}

fn abort(evaluation: Evaluation, index: usize, error: BoxError) -> BoxError {
    if let Some(outcome) = evaluation.outcomes.get(index) {
        eprint!("{}", outcome.stderr);
    }
    error
}

struct Fnv128(u128);

impl Fnv128 {
    fn new() -> Self {
        Self(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d)
    }

    fn bytes(&mut self, data: &[u8]) {
        self.raw(&(data.len() as u64).to_le_bytes());
        self.raw(data);
    }

    fn raw(&mut self, data: &[u8]) {
        for byte in data {
            self.0 ^= u128::from(*byte);
            self.0 = self
                .0
                .wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
        }
    }
}

fn cache_key(roots: &[String], sources: &[Source]) -> Result<String, BoxError> {
    let mut hash = Fnv128::new();
    hash.bytes(&std::fs::read(std::env::current_exe()?)?);
    hash.bytes(&(roots.len() as u64).to_le_bytes());
    for root in roots {
        hash.bytes(root.as_bytes());
    }
    hash.bytes(&(sources.len() as u64).to_le_bytes());
    for source in sources {
        hash.bytes(source.path.as_os_str().as_encoded_bytes());
        hash.bytes(source.text.as_bytes());
    }
    Ok(format!("{:032x}", hash.0))
}

fn read_cached(entry: &Path, mode: &str) -> Option<Outcome> {
    let status = std::fs::read_to_string(entry.join(format!("{mode}.status"))).ok()?;
    let failed = match status.as_str() {
        "fail" => true,
        "pass" => false,
        _ => return None,
    };
    Some(Outcome {
        stderr: std::fs::read_to_string(entry.join(format!("{mode}.stderr"))).ok()?,
        stdout: std::fs::read_to_string(entry.join(format!("{mode}.stdout"))).ok()?,
        failed,
    })
}

fn write_cache(dir: &Path, key: &str, outcomes: &[Outcome]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let staging = dir.join(format!(".{key}.{}", std::process::id()));
    std::fs::create_dir_all(&staging)?;
    for (mode, outcome) in MODES.iter().zip(outcomes) {
        std::fs::write(staging.join(format!("{mode}.stderr")), &outcome.stderr)?;
        std::fs::write(staging.join(format!("{mode}.stdout")), &outcome.stdout)?;
        std::fs::write(
            staging.join(format!("{mode}.status")),
            if outcome.failed { "fail" } else { "pass" },
        )?;
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path != staging && path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        }
    }
    std::fs::rename(&staging, dir.join(key))
}

fn run(mode: &str, roots: &[String], cache_dir: Option<&Path>) -> Result<Outcome, BoxError> {
    let single = [mode];
    let (modes, index): (&[&str], usize) = match cache_dir {
        Some(_) => (
            &MODES,
            MODES
                .iter()
                .position(|candidate| *candidate == mode)
                .ok_or("unknown scan mode")?,
        ),
        None => (&single, 0),
    };
    let (sources, walk_error) = collect(roots);
    if walk_error.is_some() || sources.is_empty() {
        let mut evaluation = evaluate(&sources, modes);
        let error = evaluation
            .error
            .take()
            .or(walk_error)
            .unwrap_or_else(|| "no Rust source files scanned".into());
        return Err(abort(evaluation, index, error));
    }
    let cached = match cache_dir {
        Some(dir) => Some((dir, cache_key(roots, &sources)?)),
        None => None,
    };
    if let Some((dir, key)) = &cached
        && let Some(outcome) = read_cached(&dir.join(key), mode)
    {
        return Ok(outcome);
    }
    let mut evaluation = evaluate(&sources, modes);
    if let Some(error) = evaluation.error.take() {
        return Err(abort(evaluation, index, error));
    }
    if let Some((dir, key)) = &cached {
        write_cache(dir, key, &evaluation.outcomes)?;
    }
    Ok(evaluation.outcomes.swap_remove(index))
}

fn main() -> Result<std::process::ExitCode, BoxError> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut cache_dir = None;
    if args.first().is_some_and(|arg| arg == "--cache-dir") {
        if args.len() < 2 {
            return Err("--cache-dir needs a directory".into());
        }
        cache_dir = Some(PathBuf::from(args.remove(1)));
        args.remove(0);
    }
    if args.is_empty() {
        return Err(
            "expected discarded, fail-open, tracing-messages or swallowed-errors and source roots"
                .into(),
        );
    }
    let mode = args.remove(0);
    if !MODES.contains(&mode.as_str()) {
        return Err("unknown scan mode".into());
    }
    let outcome = run(&mode, &args, cache_dir.as_deref())?;
    eprint!("{}", outcome.stderr);
    print!("{}", outcome.stdout);
    if outcome.failed {
        return Ok(std::process::ExitCode::FAILURE);
    }
    Ok(std::process::ExitCode::SUCCESS)
}
