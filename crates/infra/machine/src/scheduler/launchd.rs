//! The launchd agent property list of an entry, loaded in the user's GUI
//! domain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::entry::{EntryId, SchedulerEntry, Trigger};
use crate::quote;

#[must_use]
pub fn file_name(id: &EntryId) -> String {
    format!("{id}.plist")
}

#[must_use]
pub fn render(entry: &SchedulerEntry) -> String {
    let mut arguments = String::new();
    for argument in
        std::iter::once(entry.program()).chain(entry.arguments().iter().map(String::as_str))
    {
        arguments.push_str("\n        <string>");
        arguments.push_str(&quote::xml_text(argument));
        arguments.push_str("</string>");
    }
    let mut schedule = String::new();
    if !matches!(entry.trigger(), Trigger::OnDemand) {
        schedule.push_str("\n    <key>RunAtLoad</key>\n    <true/>");
    }
    if let Trigger::Every(interval) = entry.trigger() {
        schedule.push_str(&format!(
            "\n    <key>StartInterval</key>\n    <integer>{}</integer>",
            interval.as_seconds()
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>{arguments}
    </array>{schedule}
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
"#,
        label = quote::xml_text(entry.id().as_str()),
    )
}
