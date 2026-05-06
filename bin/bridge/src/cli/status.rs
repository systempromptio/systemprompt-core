use std::fmt::Display;
use std::process::ExitCode;

use serde::Deserialize;

use crate::auth::setup;
use crate::cli::output;
use crate::config::paths;
use crate::obs::output::diag;

fn status_line(label: &str, value: impl Display) {
    output::print_line(&format!("{label}: {value}"));
}

fn status_indent(label: &str, value: impl Display) {
    output::print_line(&format!("  {label}: {value}"));
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

    print_oauth_client_status(&s);

    if let Some(loc) = paths::org_plugins_effective() {
        print_org_plugins_status(&loc.path);
    }

    print_cowork_status();

    ExitCode::SUCCESS
}

fn print_oauth_client_status(s: &setup::StatusReport) {
    let path_display = s
        .oauth_creds_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unresolvable)".into());
    status_line("oauth client creds", path_display);
    status_indent("present", s.oauth_creds_present);
    if s.oauth_creds_present
        && let Ok(Some(creds)) = crate::auth::plugin_oauth::load_creds()
    {
        status_indent("client_id", creds.client_id);
        status_indent("token endpoint", creds.token_endpoint);
        status_indent("scopes", creds.scopes.join(" "));
    }
}

fn print_cowork_status() {
    let target = crate::integration::cowork_plugins::resolve_target();
    match target {
        Some(t) => {
            status_line("cowork session", t.session_org_dir.display());
            let known = t
                .cowork_plugins_dir
                .join(crate::integration::cowork_plugins::KNOWN_MARKETPLACES_FILE);
            status_indent(
                "known_marketplaces.json",
                if known.exists() {
                    known.display().to_string()
                } else {
                    "(not written)".into()
                },
            );
            let mp_dir = t
                .cowork_plugins_dir
                .join("marketplaces")
                .join(paths::BRIDGE_MARKETPLACE_NAME);
            status_indent(
                "bridge marketplace",
                if mp_dir.is_dir() {
                    mp_dir.display().to_string()
                } else {
                    "(not registered)".into()
                },
            );
        },
        None => {
            status_line("cowork session", "(not detected)");
        },
    }
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
