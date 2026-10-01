//! CLI value parsers for fail-fast validation at command line boundaries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{Email, ProfileName};

pub fn parse_profile_name(s: &str) -> Result<ProfileName, IdValidationError> {
    ProfileName::try_new(s)
}

pub fn parse_email(s: &str) -> Result<Email, IdValidationError> {
    Email::try_new(s)
}
