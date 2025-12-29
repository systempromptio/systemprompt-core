use anyhow::{anyhow, Result};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use super::process::run_command_in_dir;

pub fn build_docker_image(context_dir: &Path, dockerfile: &Path, image: &str) -> Result<()> {
    run_command_in_dir(
        "docker",
        &[
            "build",
            "-f",
            &dockerfile.to_string_lossy(),
            "-t",
            image,
            ".",
        ],
        &context_dir.to_path_buf(),
    )
}

pub fn docker_login(registry: &str, username: &str, password: &str) -> Result<()> {
    let mut command = Command::new("docker");
    command.args(["login", registry, "-u", username, "--password-stdin"]);
    command.stdin(std::process::Stdio::piped());

    let mut child = command.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(password.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(anyhow!("Docker login failed"));
    }

    Ok(())
}

pub fn docker_push(image: &str) -> Result<()> {
    let status = Command::new("docker").args(["push", image]).status()?;

    if !status.success() {
        return Err(anyhow!("Docker push failed for image: {}", image));
    }

    Ok(())
}
