//! Frozen, script-less Node dependency install for synced plugins, mirroring
//! what Claude Code does when it caches a plugin itself.
//!
//! The bridge writes org plugins straight into Claude Code's cache, so Claude
//! Code never sees an install step and never runs `npm ci` for them. This
//! module runs the same install — the same lockfile rule, `--ignore-scripts`,
//! the same 60-second bound — against the org-plugins copy, which every host
//! emitter then mirrors. A failed or impossible install is a warning, never a
//! failed sync: the plugin loads without its packages, exactly as it would
//! under Claude Code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use systemprompt_models::bridge::plugin_bundle::{NODE_PACKAGE_FILE, node_lockfile};

pub const DEADLINE: Duration = Duration::from_secs(60);

const STAMP: &str = ".systemprompt-install.sha256";

const STDERR_LIMIT: u64 = 64 * 1024;

const STDERR_TAIL: usize = 400;

/// What the install pass did for one plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeInstall {
    NotApplicable,
    Unchanged,
    Installed { tool: &'static str },
    Skipped { reason: String },
}

#[must_use]
pub fn binary_on_path(binary: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        ["", ".exe", ".cmd"]
            .iter()
            .map(|ext| dir.join(format!("{binary}{ext}")))
            .find(|candidate| candidate.is_file())
    })
}

fn fingerprint(plugin_dir: &Path, lockfile: &str) -> Option<String> {
    let mut bytes = Vec::new();
    for name in [NODE_PACKAGE_FILE, lockfile] {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        bytes.extend(std::fs::read(plugin_dir.join(name)).ok()?);
        bytes.push(0);
    }
    Some(crate::hash::sha256_hex(&bytes))
}

fn stamped(plugin_dir: &Path) -> Option<String> {
    std::fs::read_to_string(plugin_dir.join("node_modules").join(STAMP)).ok()
}

pub fn carry_over(installed: &Path, staged: &Path) {
    let Some(lockfile) = node_lockfile(staged) else {
        return;
    };
    let Some(expected) = fingerprint(staged, lockfile) else {
        return;
    };
    if stamped(installed).as_deref() != Some(expected.as_str()) {
        return;
    }
    if let Err(error) = std::fs::rename(installed.join("node_modules"), staged.join("node_modules"))
    {
        tracing::debug!(
            target: "bridge::sync::node",
            error = %error,
            "node_modules could not be carried over; it will be reinstalled"
        );
    }
}

pub fn install(plugin_dir: &Path) -> NodeInstall {
    if !plugin_dir.join(NODE_PACKAGE_FILE).is_file() {
        return NodeInstall::NotApplicable;
    }
    let Some(lockfile) = node_lockfile(plugin_dir) else {
        return NodeInstall::NotApplicable;
    };
    let Some(expected) = fingerprint(plugin_dir, lockfile) else {
        return NodeInstall::Skipped {
            reason: "package.json or its lockfile could not be read".to_owned(),
        };
    };
    if plugin_dir.join("node_modules").is_dir()
        && stamped(plugin_dir).as_deref() == Some(&*expected)
    {
        return NodeInstall::Unchanged;
    }
    let (tool, args): (&'static str, &[&str]) = if lockfile.starts_with("bun.") {
        ("bun", &["install", "--frozen-lockfile", "--ignore-scripts"])
    } else {
        (
            "npm",
            &["ci", "--ignore-scripts", "--no-audit", "--no-fund"],
        )
    };
    let Some(binary) = binary_on_path(tool) else {
        return NodeInstall::Skipped {
            reason: format!("{tool} is not on PATH, so {lockfile} was not installed"),
        };
    };
    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(plugin_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    crate::winproc::no_window(&mut command);
    match run_bounded(&mut command, tool) {
        Ok(()) => {
            let stamp = plugin_dir.join("node_modules").join(STAMP);
            if let Err(error) = std::fs::write(&stamp, expected) {
                return NodeInstall::Skipped {
                    reason: format!(
                        "{tool} finished but {} could not be written: {error}",
                        stamp.display()
                    ),
                };
            }
            NodeInstall::Installed { tool }
        },
        Err(reason) => NodeInstall::Skipped { reason },
    }
}

fn run_bounded(command: &mut Command, tool: &str) -> Result<(), String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("{tool} could not be started: {error}"))?;
    let Some(stderr) = child.stderr.take() else {
        let _killed = child.kill();
        let _reaped = child.wait();
        return Err(format!("{tool} started without a readable stderr"));
    };
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _read = stderr.take(STDERR_LIMIT).read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() >= DEADLINE => {
                let _killed = child.kill();
                let _reaped = child.wait();
                break Err(format!(
                    "{tool} exceeded the {}s install deadline and was stopped",
                    DEADLINE.as_secs()
                ));
            },
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _killed = child.kill();
                let _reaped = child.wait();
                break Err(format!("{tool} could not be waited on: {error}"));
            },
        }
    };
    let stderr = match reader.join() {
        Ok(stderr) => stderr,
        Err(_panicked) => String::from("(stderr could not be read)"),
    };
    let status = status?;
    if status.success() {
        return Ok(());
    }
    let tail: String = stderr
        .trim()
        .chars()
        .rev()
        .take(STDERR_TAIL)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Err(format!("{tool} exited with {status}: {tail}"))
}
