use std::fmt::Display;
use std::process::ExitCode;

use serde::Deserialize;

use crate::config::paths;
use crate::obs::output::diag;
use crate::setup;

fn status_line(label: &str, value: impl Display) {
    println!("{label}: {value}");
}

fn status_indent(label: &str, value: impl Display) {
    println!("  {label}: {value}");
}

#[derive(Deserialize)]
struct UserFragment {
    #[serde(default)]
    email: Option<String>,
}

pub(crate) fn cmd_status() -> ExitCode {
    let s = match setup::status() {
        Ok(s) => s,
        Err(e) => {
            diag(&format!("status failed: {e}"));
            return ExitCode::from(1);
        },
    };

    status_line("config file", s.paths.config_file.display());
    status_indent("present", s.config_present);
    status_line("secret file", s.paths.pat_file.display());
    status_indent("present", s.pat_present);

    if let Some(loc) = paths::org_plugins_effective() {
        print_org_plugins_status(&loc.path);
    }

    ExitCode::SUCCESS
}

fn print_org_plugins_status(plugins_path: &std::path::Path) {
    status_line("org-plugins", plugins_path.display());
    let meta = paths::metadata_dir(plugins_path);

    let last_sync = meta.join(paths::LAST_SYNC_SENTINEL);
    let last_sync_value = if last_sync.exists() {
        last_sync.display().to_string()
    } else {
        "(never)".into()
    };
    status_indent("last sync", last_sync_value);

    if let Some(email) = read_user_email(&meta) {
        status_indent("identity", email);
    }
    if let Some(n) = read_index_len(&meta.join(paths::SKILLS_DIR).join("index.json")) {
        status_indent("skills", n);
    }
    if let Some(n) = read_index_len(&meta.join(paths::AGENTS_DIR).join("index.json")) {
        status_indent("agents", n);
    }
}

fn read_user_email(meta: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(meta.join(paths::USER_FRAGMENT)).ok()?;
    let fragment: UserFragment = serde_json::from_slice(&bytes).ok()?;
    fragment.email
}

fn read_index_len(path: &std::path::Path) -> Option<usize> {
    let bytes = std::fs::read(path).ok()?;
    let entries: Vec<serde::de::IgnoredAny> = serde_json::from_slice(&bytes).ok()?;
    Some(entries.len())
}
