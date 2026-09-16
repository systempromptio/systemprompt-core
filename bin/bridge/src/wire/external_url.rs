//! Guard for URLs the GUI hands to the operating system's default browser.
//!
//! Only absolute `https://` URLs may leave the webview; every other scheme
//! (`javascript:`, `file:`, `http:`, custom protocols) is refused before the
//! target reaches any launcher.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalUrl(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExternalUrlRejected {
    #[error("refusing to open non-https url: {0}")]
    NotHttps(String),
    #[error("refusing to open url with control characters")]
    ControlCharacters,
}

impl ExternalUrl {
    pub fn parse(target: &str) -> Result<Self, ExternalUrlRejected> {
        if target.chars().any(char::is_control) {
            return Err(ExternalUrlRejected::ControlCharacters);
        }
        let scheme = target.get(..8).map(str::to_ascii_lowercase);
        if scheme.as_deref() != Some("https://") || target.len() == 8 {
            return Err(ExternalUrlRejected::NotHttps(target.to_owned()));
        }
        Ok(Self(target.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExternalUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
