use std::fmt::Display;
use std::process::ExitCode;

use crate::config::paths;
use crate::obs::output::diag;
use crate::setup;

fn status_line(label: &str, value: impl Display) {
    println!("{label}: {value}");
}

fn status_indent(label: &str, value: impl Display) {
    println!("  {label}: {value}");
}

pub(crate) fn cmd_status() -> ExitCode {
    match setup::status() {
        Ok(s) => {
            status_line("config file", s.paths.config_file.display());
            status_indent("present", s.config_present);
            status_line("secret file", s.paths.pat_file.display());
            status_indent("present", s.pat_present);
            if let Some(loc) = paths::org_plugins_effective() {
                status_line("org-plugins", loc.path.display());
                let meta = paths::metadata_dir(&loc.path);
                let last_sync = meta.join(paths::LAST_SYNC_SENTINEL);
                let last_sync_value = if last_sync.exists() {
                    last_sync.display().to_string()
                } else {
                    "(never)".into()
                };
                status_indent("last sync", last_sync_value);
                let user_file = meta.join(paths::USER_FRAGMENT);
                if let Ok(bytes) = std::fs::read(&user_file) {
                    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(email) = value.get("email").and_then(|v| v.as_str()) {
                            status_indent("identity", email);
                        }
                    }
                }
                let skills_idx = meta.join(paths::SKILLS_DIR).join("index.json");
                if let Ok(bytes) = std::fs::read(&skills_idx) {
                    if let Ok(arr) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(n) = arr.as_array().map(|a| a.len()) {
                            status_indent("skills", n);
                        }
                    }
                }
                let agents_idx = meta.join(paths::AGENTS_DIR).join("index.json");
                if let Ok(bytes) = std::fs::read(&agents_idx) {
                    if let Ok(arr) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if let Some(n) = arr.as_array().map(|a| a.len()) {
                            status_indent("agents", n);
                        }
                    }
                }
            }
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("status failed: {e}"));
            ExitCode::from(1)
        },
    }
}
