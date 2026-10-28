//! `licence` profile checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{LicenceConfig, Profile};

impl Profile {
    pub(crate) fn validate_licence(&self, errors: &mut Vec<String>) {
        let licence = &self.licence;
        if !licence.enabled() {
            return;
        }
        match url::Url::parse(&licence.endpoint) {
            Ok(parsed) if parsed.scheme() == "https" && parsed.host_str().is_some() => {},
            Ok(_) => errors.push(format!(
                "licence.endpoint must be an https:// URL with a host: {}",
                licence.endpoint
            )),
            Err(e) => errors.push(format!(
                "licence.endpoint is not a valid URL ({e}): {}",
                licence.endpoint
            )),
        }
        if licence.interval_secs < LicenceConfig::MIN_INTERVAL_SECS {
            errors.push(format!(
                "licence.interval_secs must be at least {} (got {})",
                LicenceConfig::MIN_INTERVAL_SECS,
                licence.interval_secs
            ));
        }
    }
}
