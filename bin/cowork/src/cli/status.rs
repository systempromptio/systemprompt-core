use std::fmt::Display;
use std::process::ExitCode;

use serde::Deserialize;

use crate::auth::setup;
use crate::config::paths;
use crate::obs::output::diag;

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
    let synthetic = plugins_path.join(paths::SYNTHETIC_PLUGIN_NAME);
    if let Some(n) = count_subdirs(&synthetic.join("skills")) {
        status_indent("skills", n);
    }
    if let Some(n) = count_files_with_ext(&synthetic.join("agents"), "md") {
        status_indent("agents", n);
    }
}

fn read_user_email(meta: &std::path::Path) -> Option<String> {
    let bytes = std::fs::read(meta.join(paths::USER_FRAGMENT)).ok()?;
    let fragment: UserFragment = serde_json::from_slice(&bytes).ok()?;
    fragment.email
}

fn count_subdirs(dir: &std::path::Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
            n += 1;
        }
    }
    Some(n)
}

fn count_files_with_ext(dir: &std::path::Path, ext: &str) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if entry.file_type().ok().map(|t| t.is_file()).unwrap_or(false)
            && path.extension().and_then(|e| e.to_str()) == Some(ext)
        {
            n += 1;
        }
    }
    Some(n)
}
