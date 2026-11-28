//! The systemd user units of an entry: a service, and a timer for an entry
//! that repeats.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::entry::{EntryId, SchedulerEntry, Trigger};

/// The unit files of one entry. An entry that does not repeat has no timer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdUnits {
    pub service: String,
    pub timer: Option<String>,
}

#[must_use]
pub fn service_name(id: &EntryId) -> String {
    format!("{id}.service")
}

#[must_use]
pub fn timer_name(id: &EntryId) -> String {
    format!("{id}.timer")
}

// Why: systemd expands `%` specifiers in every word of a command line and
// `$` variables in every word but the program, inside quotes too.
fn word(value: &str, program: bool) -> String {
    let mut out = String::from('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            '%' => out.push_str("%%"),
            '$' if !program => out.push_str("$$"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn command_line(entry: &SchedulerEntry) -> String {
    std::iter::once(word(entry.program(), true))
        .chain(entry.arguments().iter().map(|a| word(a, false)))
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn render(entry: &SchedulerEntry) -> SystemdUnits {
    let description = entry.description().replace('%', "%%");
    let exec = command_line(entry);
    let service = match entry.trigger() {
        Trigger::AtLogin => format!(
            "[Unit]\nDescription={description}\n\n[Service]\nType=simple\nExecStart={exec}\n\n[Install]\nWantedBy=default.target\n"
        ),
        Trigger::Every(_) | Trigger::OnDemand => format!(
            "[Unit]\nDescription={description}\n\n[Service]\nType=oneshot\nExecStart={exec}\n"
        ),
    };
    let timer = match entry.trigger() {
        Trigger::Every(interval) => Some(format!(
            "[Unit]\nDescription={description}\n\n[Timer]\nOnStartupSec=1min\nOnUnitActiveSec={minutes}min\nUnit={unit}\n\n[Install]\nWantedBy=timers.target\n",
            minutes = interval.as_minutes(),
            unit = service_name(entry.id()),
        )),
        Trigger::AtLogin | Trigger::OnDemand => None,
    };
    SystemdUnits { service, timer }
}
