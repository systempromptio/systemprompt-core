mod config;
mod install;
mod managed_resources;
mod probe;

pub use managed_resources::CodexCliSync;

use crate::integration::host_app::{
    ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, HostKind,
    ProfileGenInputs, ProfileState,
};

pub struct CodexCliHost;

pub static CODEX_CLI_HOST: CodexCliHost = CodexCliHost;

impl HostApp for CodexCliHost {
    fn id(&self) -> &'static str {
        "codex-cli"
    }

    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }

    fn config_schema(&self) -> &'static HostConfigSchema {
        &config::SCHEMA
    }

    fn probe(&self) -> HostAppSnapshot {
        let read = probe::read_config();
        let profile_state = ProfileState::from_keys(config::REQUIRED_KEYS, &read.keys);
        let processes = probe::list_codex_processes();
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state,
            profile_source: read.source_path,
            profile_keys: read.keys,
            host_running: !processes.is_empty(),
            host_processes: processes,
            probed_at_unix: config::now_unix(),
        }
    }

    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        install::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> std::io::Result<()> {
        install::install_profile(path)
    }

    /// Codex is a CLI; "open" launches a terminal so the user can run `codex` interactively.
    fn open(&self) -> std::io::Result<()> {
        open_terminal()
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "loaded into managed preferences (com.openai.codex)"
        } else if cfg!(target_os = "windows") {
            "merged into %USERPROFILE%\\.codex\\managed_config.toml"
        } else {
            "merged into /etc/codex/config.toml"
        }
    }

    fn kind(&self) -> HostKind {
        HostKind::CliTool
    }

    fn description(&self) -> &'static str {
        "OpenAI's Codex (CLI, desktop app, IDE extension). systemprompt-bridge installs managed \
         configuration that takes precedence over user config across all three surfaces."
    }

    fn icon_id(&self) -> &'static str {
        "codex-cli"
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }
}

#[cfg(target_os = "macos")]
fn open_terminal() -> std::io::Result<()> {
    let status = std::process::Command::new("/usr/bin/open")
        .args(["-a", "Terminal"])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "open -a Terminal exited with {}",
            status.code().unwrap_or(-1)
        )))
    }
}

#[cfg(target_os = "windows")]
fn open_terminal() -> std::io::Result<()> {
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", "codex"])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "start cmd exited with {}",
            status.code().unwrap_or(-1)
        )))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_terminal() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "open not supported on this platform",
    ))
}
