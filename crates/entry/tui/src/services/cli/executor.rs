use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::messages::Message;

#[derive(Debug)]
pub struct CliExecutor {
    tx: mpsc::UnboundedSender<Message>,
}

impl CliExecutor {
    pub const fn new(tx: mpsc::UnboundedSender<Message>) -> Self {
        Self { tx }
    }

    pub fn spawn_execution(&self, command_string: String) {
        let tx = self.tx.clone();

        tokio::spawn(async move {
            let result = execute_command(&command_string).await;
            match result {
                Ok(output) => {
                    let _ = tx.send(Message::CommandCliOutput(output));
                },
                Err(e) => {
                    let _ = tx.send(Message::CommandCliError(e));
                },
            }
        });
    }
}

async fn execute_command(command_string: &str) -> Result<String, String> {
    let parts: Vec<&str> = command_string.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let program = parts[0];
    let args = &parts[1..];

    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn process: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture stderr".to_string())?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut output_lines = Vec::new();
    let mut error_lines = Vec::new();

    loop {
        tokio::select! {
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(line)) => output_lines.push(line),
                    Ok(None) => break,
                    Err(e) => {
                        error_lines.push(format!("Read error: {e}"));
                        break;
                    }
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(line)) => error_lines.push(line),
                    Ok(None) => {},
                    Err(e) => {
                        error_lines.push(format!("Read error: {e}"));
                    }
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for process: {e}"))?;

    if status.success() {
        if output_lines.is_empty() && !error_lines.is_empty() {
            Ok(error_lines.join("\n"))
        } else {
            Ok(output_lines.join("\n"))
        }
    } else {
        let error_output = if error_lines.is_empty() {
            output_lines.join("\n")
        } else {
            error_lines.join("\n")
        };

        Err(format!(
            "Command failed with exit code {}: {}",
            status.code().unwrap_or(-1),
            error_output
        ))
    }
}
