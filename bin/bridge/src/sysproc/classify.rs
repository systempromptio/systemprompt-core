//! Which packaging of a host product a running process belongs to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostAppKind;

use super::{SysprocError, list_processes};

/// One running process of a host tool: where it runs from and which packaging
/// of the product it is, so each host filters the hits that are its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessHit {
    pub path: String,
    pub packaging: HostAppKind,
}

// Why: a desktop app and its CLI share an executable name (`codex`,
// `opencode`), so the packaging is read off the executable's location: a macOS
// bundle (`/<Binary>.app/`) or one of the installer folders the host names.
#[must_use]
pub fn classify_process(
    binary: &str,
    name: &str,
    path: Option<&str>,
    app_markers: &[&str],
) -> Option<ProcessHit> {
    let exe = format!("{binary}.exe");
    let suffix = format!("/{binary}");
    let bundle = format!("/{binary}.app/");
    let name_lower = name.to_ascii_lowercase();
    let path_lower = path.map(str::to_ascii_lowercase).unwrap_or_default();
    let matched = if cfg!(target_os = "windows") {
        name_lower == exe
    } else {
        path_lower.ends_with(&suffix) || path_lower.contains(&bundle) || name_lower == binary
    };
    if !matched {
        return None;
    }
    let is_app = path_lower.contains(&bundle)
        || app_markers
            .iter()
            .any(|marker| path_lower.contains(&marker.to_ascii_lowercase()));
    Some(ProcessHit {
        path: if path_lower.is_empty() {
            name_lower
        } else {
            path_lower
        },
        packaging: if is_app {
            HostAppKind::DesktopApp
        } else {
            HostAppKind::CliTool
        },
    })
}

pub(crate) fn find_processes(
    binary: &str,
    app_markers: &[&str],
) -> Result<Vec<ProcessHit>, SysprocError> {
    let mut hits: Vec<ProcessHit> = list_processes()?
        .into_iter()
        .filter_map(|p| classify_process(binary, &p.name, p.path.as_deref(), app_markers))
        .collect();
    hits.sort_by(|a, b| a.path.cmp(&b.path));
    hits.dedup();
    Ok(hits)
}
