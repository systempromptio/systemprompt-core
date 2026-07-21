//! Per-OS scheduled-task templates (launchd, Task Scheduler, systemd) for the
//! sync agent.
//!
//! Every identifier the templates carry — the launchd `Label`, the Task
//! Scheduler task name, the systemd unit basename, and the human-readable
//! description — is brand-scoped (see [`crate::brand`]), so a white-label
//! bridge never registers a job under the upstream `systemprompt` name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::brand::brand;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "macos" | "darwin" | "mac" => Some(Self::Mac),
            "windows" | "win" => Some(Self::Windows),
            "linux" => Some(Self::Linux),
            _ => None,
        }
    }
}

/// The brand-scoped identifier the scheduler registers this job under: a
/// reverse-DNS launchd label, a Task Scheduler task name, or a systemd unit
/// basename.
#[must_use]
pub fn schedule_label(os: Os) -> &'static str {
    let brand = brand();
    match os {
        Os::Mac => brand.schedule_label,
        Os::Windows => brand.schedule_task_name,
        Os::Linux => brand.schedule_unit,
    }
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "{binary}/{label}/{app} are template placeholders consumed by str::replace, not fmt args"
)]
#[must_use]
pub fn template(os: Os, binary: &Path) -> String {
    let tmpl = match os {
        Os::Mac => LAUNCHD_PLIST_TMPL,
        Os::Windows => TASK_SCHEDULER_XML_TMPL,
        Os::Linux => SYSTEMD_UNIT_TMPL,
    };
    tmpl.replace("{binary}", &binary.display().to_string())
        .replace("{label}", schedule_label(os))
        .replace("{app}", brand().app_name)
}

#[must_use]
pub fn template_filename(os: Os) -> String {
    let brand = brand();
    match os {
        Os::Mac => format!("{}.plist", brand.schedule_label),
        Os::Windows => format!("{}.xml", brand.schedule_unit),
        Os::Linux => format!("{}.service+timer", brand.schedule_unit),
    }
}

/// Splits the combined Linux template into its `.service` and `.timer` bodies,
/// which systemd requires as two separate unit files.
///
/// Returns `None` if the timer section marker is absent.
#[must_use]
pub fn split_systemd_unit(rendered: &str) -> Option<(String, String)> {
    let timer_start = rendered.match_indices("# === ").find_map(|(idx, _)| {
        rendered[idx..]
            .lines()
            .next()
            .is_some_and(|line| line.contains(".timer ==="))
            .then_some(idx)
    })?;
    let service = rendered[..timer_start].trim_end();
    let timer = rendered[timer_start..].trim_end();
    Some((format!("{service}\n"), format!("{timer}\n")))
}

#[must_use]
pub fn install_hint(os: Os) -> String {
    let brand = brand();
    let filename = template_filename(os);
    match os {
        Os::Mac => format!(
            "Save to ~/Library/LaunchAgents/{filename}, then: launchctl bootstrap gui/$(id -u) \
             ~/Library/LaunchAgents/{filename}"
        ),
        Os::Windows => format!(
            "Save as {filename}, then: schtasks /Create /TN \"{}\" /XML {filename}",
            brand.schedule_task_name
        ),
        Os::Linux => format!(
            "Split into ~/.config/systemd/user/{unit}.{{service,timer}}, then: systemctl --user \
             daemon-reload && systemctl --user enable --now {unit}.timer",
            unit = brand.schedule_unit
        ),
    }
}

const LAUNCHD_PLIST_TMPL: &str = include_str!("templates/launchd.plist.tmpl");
const TASK_SCHEDULER_XML_TMPL: &str = include_str!("templates/task-scheduler.xml.tmpl");
const SYSTEMD_UNIT_TMPL: &str = include_str!("templates/systemd.unit.tmpl");
