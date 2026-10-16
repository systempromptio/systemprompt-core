//! Masking helpers for secrets and database URLs shown in profile output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Profile;

impl Profile {
    pub fn mask_secret(value: &str, visible_chars: usize) -> String {
        if value.is_empty() {
            return "(not set)".to_owned();
        }
        if value.len() <= visible_chars {
            return "***".to_owned();
        }
        format!("{}...", &value[..visible_chars])
    }

    pub fn mask_database_url(url: &str) -> String {
        if let Some(at_pos) = url.find('@')
            && let Some(colon_pos) = url[..at_pos].rfind(':')
        {
            let prefix = &url[..=colon_pos];
            let suffix = &url[at_pos..];
            return format!("{}***{}", prefix, suffix);
        }
        url.to_owned()
    }

    pub fn is_masked_database_url(url: &str) -> bool {
        url.contains(":***@") || url.contains(":********@")
    }
}
