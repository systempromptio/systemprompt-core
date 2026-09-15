//! Which hive holds the Claude policy, judged the way Claude reads it: a
//! machine key shadows the per-user copy whenever both exist.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ConfigStore, PolicyDocument, PolicyHive, PolicyTarget};

const PROBE_KEYS: &[&str] = &["inferenceGatewayBaseUrl", "managedMcpServers"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HiveReport {
    Unwritten,
    Machine,
    User,
    UserBeneathElevatedWriter,
    Matching,
    Shadowed,
}

impl HiveReport {
    #[must_use]
    pub fn classify(machine: &PolicyDocument, user: &PolicyDocument, elevated: bool) -> Self {
        match (machine.is_empty(), user.is_empty()) {
            (true, true) => Self::Unwritten,
            (false, true) => Self::Machine,
            (true, false) if elevated => Self::UserBeneathElevatedWriter,
            (true, false) => Self::User,
            (false, false) if machine == user => Self::Matching,
            (false, false) => Self::Shadowed,
        }
    }

    #[must_use]
    pub const fn detail(self) -> &'static str {
        match self {
            Self::Unwritten => "no Claude policy in HKLM or HKCU — sync has not written one yet",
            Self::Machine => "HKLM holds the Claude policy (machine-wide)",
            Self::User => {
                "HKCU holds the Claude policy (per-user; honoured while no HKLM policy exists)"
            },
            Self::UserBeneathElevatedWriter => {
                "HKCU holds the Claude policy but this process is elevated and will write HKLM, \
                 which then shadows the per-user copy"
            },
            Self::Matching => "HKLM and HKCU both hold the Claude policy with matching values",
            Self::Shadowed => {
                "HKLM shadows a different HKCU policy — Claude reads HKLM only; repair as \
                 administrator from the app, or re-run `install --apply` as Administrator"
            },
        }
    }

    #[must_use]
    pub const fn is_failure(self) -> bool {
        matches!(self, Self::Shadowed)
    }

    #[must_use]
    pub const fn is_warning(self) -> bool {
        matches!(self, Self::Unwritten | Self::UserBeneathElevatedWriter)
    }
}

pub fn hive_report(store: &dyn ConfigStore, elevated: bool) -> Result<HiveReport, String> {
    let read = |hive: PolicyHive| {
        store
            .read_policy_document(hive, PolicyTarget::Claude, PROBE_KEYS)
            .map_err(|e| format!("{}: {e}", hive.label()))
    };
    let machine = read(PolicyHive::Machine)?;
    let user = read(PolicyHive::User)?;
    Ok(HiveReport::classify(&machine, &user, elevated))
}
