//! Pinned client checks run without credentials, workspace mounts, or
//! networking.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ContainerLaunch, NativeClient, SchedulerError, SchedulerResult, docker_command, private_log,
    safe_name, wait_bounded,
};
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};
use systemprompt_loader::subprocess::{place_in_own_process_group, spawn_owned_supervised};

pub trait ClientVerifier: Send + Sync + std::fmt::Debug {
    fn verify(&self, launch: &ContainerLaunch, client: &NativeClient) -> SchedulerResult<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct PinnedClientVerifier;

impl ClientVerifier for PinnedClientVerifier {
    fn verify(&self, launch: &ContainerLaunch, client: &NativeClient) -> SchedulerResult<()> {
        launch.verify_client(client)
    }
}

impl ContainerLaunch {
    fn verify_client(&self, client: &NativeClient) -> SchedulerResult<()> {
        let target = client.admitted_target(&self.image).map_err(failure)?;
        let expected_config =
            systemprompt_evaluation::capabilities::proofs::image_config_for_target(
                target,
                &self.image,
            )
            .map_err(failure)?;
        self.verify_image_config(expected_config)?;
        let adapter = client.adapter().map_err(failure)?;
        if adapter.adapter_version() != target.adapter_version
            || !Path::new(adapter.executable()).is_absolute()
        {
            return Err(failure(
                "Adapter version or absolute executable path does not match native admission",
            ));
        }
        let digest = self.probe_client(
            "digest",
            "/usr/bin/sha256sum",
            &[OsString::from(adapter.executable())],
        )?;
        let digest = std::str::from_utf8(&digest).map_err(failure)?;
        if digest.split_whitespace().next() != Some(target.executable_digest.as_str()) {
            return Err(failure(
                "Pinned executable bytes do not match native admission",
            ));
        }
        let version = self.probe_client(
            "version",
            adapter.executable(),
            &adapter.version_arguments(),
        )?;
        if adapter.parse_version(&version).map_err(failure)? != target.client_version {
            return Err(failure(
                "Observed executable version does not match native admission",
            ));
        }
        let path = self
            .directory
            .join(format!("{}-native-verification.json", self.output_stem));
        private_log(&path)?.write_all(&serde_json::to_vec(target).map_err(failure)?)?;
        Ok(())
    }

    fn verify_image_config(&self, expected: &str) -> SchedulerResult<()> {
        let output = self
            .directory
            .join(format!("{}-image-identity.stdout", self.output_stem));
        let (mut command, _configuration) = docker_command(&self.docker)?;
        command
            .args(["image", "inspect", "--format", "{{.Id}}", &self.image])
            .stdin(Stdio::null())
            .stdout(Stdio::from(private_log(&output)?))
            .stderr(Stdio::null());
        place_in_own_process_group(&mut command);
        let mut child = spawn_owned_supervised(command)?;
        let status = wait_bounded(&mut child)?;
        if !status.success() || std::fs::metadata(&output)?.len() > 256 {
            return Err(failure(
                "Pinned image configuration could not be established",
            ));
        }
        let observed = std::fs::read_to_string(&output)?;
        if observed.trim() != format!("sha256:{expected}") {
            return Err(failure(
                "Image manifest resolved to a different retained config identity",
            ));
        }
        Ok(())
    }

    pub fn probe_client(
        &self,
        label: &str,
        executable: &str,
        arguments: &[OsString],
    ) -> SchedulerResult<Vec<u8>> {
        if !safe_name(label)
            || label.len() > 32
            || !Path::new(executable).is_absolute()
            || arguments.len() > 64
            || arguments
                .iter()
                .map(|argument| argument.len())
                .sum::<usize>()
                > 16_384
        {
            return Err(failure("Invalid bounded native probe command"));
        }
        let name = format!("{}-pin-{label}", self.name);
        let output = self
            .directory
            .join(format!("{}-pin-{label}.stdout", self.output_stem));
        let errors = self
            .directory
            .join(format!("{}-pin-{label}.stderr", self.output_stem));
        let (mut command, _docker_configuration) = docker_command(&self.docker)?;
        command.args([
            "run",
            "--pull=never",
            "--name",
            &name,
            "--label",
            "systemprompt.evaluator=true",
            "--label",
            &format!("systemprompt.evaluator.owner={}", self.owner_label),
            "--label",
            &format!("systemprompt.evaluator.execution={}", self.execution_label),
            "--network=none",
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--pids-limit=32",
            "--memory=256m",
            "--cpus=1",
            "--user",
            &self.runtime_user,
            "--tmpfs=/tmp:rw,nosuid,nodev,size=16m",
            "--env=HOME=/tmp",
            "--entrypoint",
            executable,
        ]);
        if let Some(lease) = &self.lease {
            command.args([
                "--label",
                &format!("systemprompt.evaluator.worker={}", lease.worker_id),
                "--label",
                &format!("systemprompt.evaluator.fence={}", lease.fencing_token),
            ]);
        }
        command.arg(&self.image);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::from(private_log(&output)?))
            .stderr(Stdio::from(private_log(&errors)?));
        place_in_own_process_group(&mut command);
        let mut child = spawn_owned_supervised(command)?;
        let result = await_probe(&mut child, &output, &errors);
        let cleanup = self.cleanup_probe(&name, &mut child);
        match (result, cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(primary), Ok(())) => Err(failure(format!("{primary}; probe cleanup confirmed"))),
            (Ok(_), Err(cleanup)) => Err(cleanup),
            (Err(primary), Err(cleanup)) => Err(failure(format!(
                "{primary}; probe cleanup failed: {cleanup}"
            ))),
        }
    }

    fn cleanup_probe(&self, name: &str, child: &mut Child) -> SchedulerResult<()> {
        let removed = (|| -> SchedulerResult<()> {
            let (mut cleanup, _docker_configuration) = docker_command(&self.docker)?;
            cleanup
                .args(["rm", "--force", name])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut cleanup = spawn_owned_supervised(cleanup)?;
            if !wait_bounded(&mut cleanup)?.success() {
                return Err(failure(
                    "Native pin verification container cleanup was not acknowledged",
                ));
            }
            Ok(())
        })();
        let reaped = if let Ok(Some(_)) = child.try_wait() {
            Ok(())
        } else {
            let killed = child.kill();
            let waited = wait_bounded(child);
            match (killed, waited) {
                (_, Ok(_)) => Ok(()),
                (Ok(()), Err(error)) => Err(failure(error)),
                (Err(kill), Err(wait)) => {
                    Err(failure(format!("Probe termination: {kill}; reap: {wait}")))
                },
            }
        };
        match (removed, reaped) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(container), Err(process)) => Err(failure(format!("{container}; {process}"))),
        }
    }
}

fn await_probe(child: &mut Child, output: &Path, errors: &Path) -> SchedulerResult<Vec<u8>> {
    let started = Instant::now();
    loop {
        let size = std::fs::metadata(output)?
            .len()
            .saturating_add(std::fs::metadata(errors)?.len());
        if size > 65_536 || started.elapsed() > Duration::from_secs(10) {
            return Err(failure(
                "Native pin verification exceeded time or output bounds",
            ));
        }
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Err(failure("Native pin verification command failed"));
            }
            return Ok(std::fs::read(output)?);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn failure(error: impl std::fmt::Display) -> SchedulerError {
    SchedulerError::config_error(error.to_string())
}
