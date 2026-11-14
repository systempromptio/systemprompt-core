//! The identity of the binary a node runs.
//!
//! Core's version alone cannot tell two downstream builds on the same core
//! release apart, so an embedding binary names itself — product name, its
//! own version, and optionally the source revision it was built from — and
//! the health and probe bodies report it under `build` next to core's
//! `version`. A binary that names nothing reports core itself
//! ([`BuildIdentity::core`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

/// Name, version and source revision of the binary serving a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildIdentity {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

impl BuildIdentity {
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            revision: None,
        }
    }

    #[must_use]
    pub fn with_revision(mut self, revision: impl Into<String>) -> Self {
        self.revision = Some(revision.into());
        self
    }

    #[must_use]
    pub fn core() -> Self {
        Self::new("systemprompt", env!("CARGO_PKG_VERSION"))
    }
}

impl Default for BuildIdentity {
    fn default() -> Self {
        Self::core()
    }
}
