//! The Task Scheduler definition of an entry: a hidden task that runs as the
//! signed-in user with their ordinary rights.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::entry::{SchedulerEntry, Trigger};
use crate::quote;

const LOGON: &str = "
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>";

// Why: Task Scheduler refuses a time trigger without a start boundary; any
// past instant makes the repetition run from registration on.
const START_BOUNDARY: &str = "2026-01-01T00:00:00";

fn triggers(trigger: Trigger) -> String {
    match trigger {
        Trigger::AtLogin => format!("<Triggers>{LOGON}\n  </Triggers>"),
        Trigger::Every(interval) => format!(
            "<Triggers>{LOGON}
    <TimeTrigger>
      <Repetition>
        <Interval>PT{}M</Interval>
      </Repetition>
      <StartBoundary>{START_BOUNDARY}</StartBoundary>
      <Enabled>true</Enabled>
    </TimeTrigger>
  </Triggers>",
            interval.as_minutes()
        ),
        Trigger::OnDemand => "<Triggers />".to_owned(),
    }
}

#[must_use]
pub fn render(entry: &SchedulerEntry) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{description}</Description>
  </RegistrationInfo>
  {triggers}
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <AllowHardTerminate>true</AllowHardTerminate>
    <Hidden>true</Hidden>
    <StartWhenAvailable>true</StartWhenAvailable>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <Arguments>{arguments}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        description = quote::xml_text(entry.description()),
        triggers = triggers(entry.trigger()),
        command = quote::xml_text(entry.program()),
        arguments = quote::xml_text(&quote::windows_command_line(entry.arguments())),
    )
}

// Why: the definition declares UTF-16, and the scheduler's import reads the
// byte order mark to tell which.
#[must_use]
pub fn encode(xml: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}
