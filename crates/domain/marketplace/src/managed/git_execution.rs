//! Bounded Git subprocesses with source-scoped credentials and no ambient
//! configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{ManagedError, Result};

const DEADLINE: Duration = Duration::from_secs(60);
const OUTPUT_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct GitExecutionLimits {
    pub deadline: Duration,
    pub output_bytes: u64,
}

impl Default for GitExecutionLimits {
    fn default() -> Self {
        Self {
            deadline: DEADLINE,
            output_bytes: OUTPUT_LIMIT,
        }
    }
}

pub fn execute(
    command: &mut Command,
    credential: Option<(&str, &str)>,
    limits: GitExecutionLimits,
) -> Result<Vec<u8>> {
    if limits.deadline.is_zero()
        || limits.deadline > DEADLINE
        || limits.output_bytes == 0
        || limits.output_bytes > OUTPUT_LIMIT
    {
        return Err(super::error::invalid("Invalid Git execution bounds"));
    }
    configure(command, credential)?;
    let working_directory = command.get_current_dir().map(std::path::Path::to_path_buf);
    let mut child = command.spawn().map_err(|_error| failed())?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        terminate(&mut child)?;
        return Err(failed());
    };
    let (sender, receiver) = mpsc::channel();
    let mut readers = Vec::new();
    let streams: [(bool, Box<dyn Read + Send>, u64); 2] = [
        (true, Box::new(stdout), limits.output_bytes),
        (false, Box::new(stderr), 64 * 1024),
    ];
    for (is_stdout, stream, limit) in streams {
        let sender = sender.clone();
        readers.push(std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stream.take(limit + 1).read_to_end(&mut bytes);
            let valid = result.is_ok() && bytes.len() as u64 <= limit;
            let _sent = sender.send((is_stdout, valid, if is_stdout { bytes } else { Vec::new() }));
        }));
    }
    drop(sender);
    let result = collect(&mut child, &receiver, working_directory.as_deref(), limits);
    let cleanup = terminate(&mut child);
    cleanup?;
    for reader in readers {
        reader.join().map_err(|_error| failed())?;
    }
    result
}

fn collect(
    child: &mut Child,
    receiver: &mpsc::Receiver<(bool, bool, Vec<u8>)>,
    working_directory: Option<&std::path::Path>,
    limits: GitExecutionLimits,
) -> Result<Vec<u8>> {
    let started = Instant::now();
    let mut output = None;
    let mut completed = 0;
    let mut disk_check = Instant::now();
    loop {
        while let Ok((is_stdout, valid, bytes)) = receiver.try_recv() {
            if !valid {
                return Err(failed());
            }
            completed += 1;
            if is_stdout {
                output = Some(bytes);
            }
        }
        if let Some(status) = child.try_wait().map_err(|_error| failed())? {
            if !status.success() {
                return Err(failed());
            }
            if completed == 2 {
                return output.ok_or_else(failed);
            }
        }
        if disk_check.elapsed() >= Duration::from_millis(100) {
            if let Some(directory) = &working_directory
                && !bounded_disk(directory)
            {
                return Err(failed());
            }
            disk_check = Instant::now();
        }
        if started.elapsed() >= limits.deadline {
            return Err(failed());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn terminate(child: &mut Child) -> Result<()> {
    #[cfg(unix)]
    {
        let status = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_error| failed())?;
        if !status.success() && child.try_wait().map_err(|_error| failed())?.is_none() {
            return Err(failed());
        }
    }
    #[cfg(not(unix))]
    child.kill().map_err(|_error| failed())?;
    child.wait().map_err(|_error| failed())?;
    Ok(())
}

fn failed() -> ManagedError {
    ManagedError::Conflict("Git operation failed or exceeded its execution/output limit; no publication selection changed".to_owned())
}

fn bounded_disk(root: &std::path::Path) -> bool {
    let mut pending = vec![root.to_path_buf()];
    let mut total = 0u64;
    let mut entries = 0usize;
    while let Some(directory) = pending.pop() {
        let Ok(children) = std::fs::read_dir(directory) else {
            return false;
        };
        for child in children {
            let Ok(child) = child else {
                return false;
            };
            let Ok(metadata) = child.path().symlink_metadata() else {
                return false;
            };
            entries += 1;
            total = total.saturating_add(metadata.len());
            if entries > 8192 || total > 64 * 1024 * 1024 || metadata.is_symlink() {
                return false;
            }
            if metadata.is_dir() {
                pending.push(child.path());
            }
        }
    }
    true
}

pub(crate) fn create_private_directory(path: &std::path::Path) -> Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

fn configure(command: &mut Command, credential: Option<(&str, &str)>) -> Result<()> {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", null)
        .env("XDG_CONFIG_HOME", null)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_SYSTEM", null)
        .env("GIT_CONFIG_GLOBAL", null)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ALLOW_PROTOCOL", "https")
        .env(
            "GIT_CONFIG_COUNT",
            if credential.is_some() { "8" } else { "7" },
        )
        .env("GIT_CONFIG_KEY_0", "credential.helper")
        .env("GIT_CONFIG_VALUE_0", "")
        .env("GIT_CONFIG_KEY_1", "http.followRedirects")
        .env("GIT_CONFIG_VALUE_1", "false")
        .env("GIT_CONFIG_KEY_2", "core.hooksPath")
        .env("GIT_CONFIG_VALUE_2", null)
        .env("GIT_CONFIG_KEY_3", "init.templateDir")
        .env("GIT_CONFIG_VALUE_3", "")
        .env("GIT_CONFIG_KEY_4", "http.lowSpeedLimit")
        .env("GIT_CONFIG_VALUE_4", "1024")
        .env("GIT_CONFIG_KEY_5", "http.lowSpeedTime")
        .env("GIT_CONFIG_VALUE_5", "15")
        .env("GIT_CONFIG_KEY_6", "fetch.recurseSubmodules")
        .env("GIT_CONFIG_VALUE_6", "false")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some((repository, token)) = credential {
        if token.is_empty() || token.len() > 8192 || token.chars().any(char::is_control) {
            return Err(super::error::invalid("Invalid resolved Git credential"));
        }
        command
            .env("GIT_CONFIG_KEY_7", format!("http.{repository}.extraHeader"))
            .env(
                "GIT_CONFIG_VALUE_7",
                format!("Authorization: Bearer {token}"),
            );
    }
    if !cfg!(unix) {
        return Err(super::error::invalid(
            "Git source execution requires verified Unix process-group cleanup",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    Ok(())
}
