//! What a host probe reads out of a config file: where it came from and the
//! keys of interest it carried. One definition for every host, so a reader
//! that adds a fact adds it for all of them.
//!
//! `keys` is what the GUI shows (sensitive values redacted); `observed` holds
//! the same keys unredacted, for the drift comparison only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub(crate) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
    pub observed: BTreeMap<String, String>,
    pub probe_error: Option<String>,
}

impl DomainRead {
    pub(crate) fn collect(
        source: &str,
        keys_of_interest: &[&str],
        lookup: impl Fn(&str) -> Option<String>,
        redact: impl Fn(&str, String) -> String,
    ) -> Self {
        let mut out = Self {
            source_path: Some(source.to_owned()),
            ..Self::default()
        };
        for dotted in keys_of_interest {
            if let Some(raw) = lookup(dotted) {
                out.observed.insert((*dotted).to_owned(), raw.clone());
                out.keys.insert((*dotted).to_owned(), redact(dotted, raw));
            }
        }
        out
    }

    pub(crate) fn unreadable(source: &str, error: &dyn std::fmt::Display) -> Self {
        Self {
            source_path: Some(source.to_owned()),
            probe_error: Some(format!("{source}: {error}")),
            ..Self::default()
        }
    }
}

pub(crate) use crate::host_sync::ForeignShape;

impl DomainRead {
    pub(crate) fn desired(
        &self,
        roles: impl Fn(&str) -> crate::drift::EntryRole,
    ) -> crate::drift::DesiredProfile {
        let mut desired = crate::drift::DesiredProfile::new();
        for (key, value) in &self.observed {
            desired.push(key.clone(), value.clone(), roles(key));
        }
        desired
    }
}
