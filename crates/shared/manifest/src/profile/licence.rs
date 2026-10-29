//! Profile `licence:` block — the report-only licence check-in.
//!
//! A multi-tenant deployment periodically reports what it runs (tenant,
//! hosting provider, live replicas per role, core version) to `endpoint`,
//! signed with the `licence_signing_secret` secret. The check-in only reports:
//! a failed or refused check-in never blocks a request or degrades service.
//! The block is inert unless `multi_tenant` is `true`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::EnterpriseLicenseId;

pub const LICENCE_SIGNING_SECRET: &str = "licence_signing_secret";

/// Where and how often a multi-tenant deployment reports its licence check-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LicenceConfig {
    #[serde(default)]
    pub multi_tenant: bool,

    #[serde(default)]
    pub licence_id: Option<EnterpriseLicenseId>,

    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
}

impl LicenceConfig {
    pub const DEFAULT_ENDPOINT: &'static str = "https://api.systemprompt.io/api/v1/licence/checkin";
    pub const DEFAULT_INTERVAL_SECS: u64 = 21_600;
    pub const MIN_INTERVAL_SECS: u64 = 3_600;

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.multi_tenant
    }

    #[must_use]
    pub fn effective_interval_secs(&self) -> u64 {
        self.interval_secs.max(Self::MIN_INTERVAL_SECS)
    }
}

impl Default for LicenceConfig {
    fn default() -> Self {
        Self {
            multi_tenant: false,
            licence_id: None,
            endpoint: default_endpoint(),
            interval_secs: default_interval_secs(),
        }
    }
}

fn default_endpoint() -> String {
    LicenceConfig::DEFAULT_ENDPOINT.to_owned()
}

const fn default_interval_secs() -> u64 {
    LicenceConfig::DEFAULT_INTERVAL_SECS
}
