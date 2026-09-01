//! What a host probe reads out of a config file: where it came from and the
//! keys of interest it carried. One definition for every host, so a reader
//! that adds a fact adds it for all of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub(crate) struct DomainRead {
    pub source_path: Option<String>,
    pub keys: BTreeMap<String, String>,
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
            keys: BTreeMap::new(),
        };
        for dotted in keys_of_interest {
            if let Some(raw) = lookup(dotted) {
                out.keys.insert((*dotted).to_owned(), redact(dotted, raw));
            }
        }
        out
    }
}
