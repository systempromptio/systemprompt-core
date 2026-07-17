//! CLI value parsers for fail-fast validation at command line boundaries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{Email, ProfileName};

pub fn parse_profile_name(s: &str) -> Result<ProfileName, String> {
    ProfileName::try_new(s).map_err(|e| e.to_string())
}

pub fn parse_email(s: &str) -> Result<Email, String> {
    Email::try_new(s).map_err(|e| e.to_string())
}
