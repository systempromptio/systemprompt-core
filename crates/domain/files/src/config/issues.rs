//! The `files.issues` section: bug and feedback reports (issue reports).
//!
//! Off by default. `maxReportsPerHour` bounds the new reports one reporter
//! may file in an hour (a repeat of an open report folds into it and is not
//! counted), `maxAttachments` bounds the files one report carries,
//! `retentionDays` is how long a resolved or closed report is kept, and
//! `triageRoles` are the roles that see and triage every report. Attachments
//! are document-store uploads, so they need `files.documents` on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

const DEFAULT_MAX_REPORTS_PER_HOUR: u32 = 20;
const DEFAULT_MAX_ATTACHMENTS: u32 = 5;
const DEFAULT_RETENTION_DAYS: u32 = 90;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuesConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_max_reports_per_hour")]
    pub max_reports_per_hour: u32,
    #[serde(default = "default_max_attachments")]
    pub max_attachments: u32,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_triage_roles")]
    pub triage_roles: Vec<String>,
}

const fn default_max_reports_per_hour() -> u32 {
    DEFAULT_MAX_REPORTS_PER_HOUR
}

const fn default_max_attachments() -> u32 {
    DEFAULT_MAX_ATTACHMENTS
}

const fn default_retention_days() -> u32 {
    DEFAULT_RETENTION_DAYS
}

fn default_triage_roles() -> Vec<String> {
    vec!["admin".to_owned()]
}

impl Default for IssuesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_reports_per_hour: DEFAULT_MAX_REPORTS_PER_HOUR,
            max_attachments: DEFAULT_MAX_ATTACHMENTS,
            retention_days: DEFAULT_RETENTION_DAYS,
            triage_roles: default_triage_roles(),
        }
    }
}

impl IssuesConfig {
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn is_triager(&self, roles: &[String]) -> bool {
        roles.iter().any(|role| self.triage_roles.contains(role))
    }
}
