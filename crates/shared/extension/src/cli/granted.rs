//! The narrow values a reserved-path command reaches through a kernel grant.
//!
//! A member command reaches kernel-held material only through a grant listed
//! on its reserved CLI path's row. Each grant hands out one purpose-typed
//! value, never the profile, the configuration or a secrets source:
//! [`ServicesTree`] for `KernelGrant::ServicesTree` and [`CallerSession`]
//! for `KernelGrant::CallerSession`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_identifiers::SessionToken;

/// The active services tree and the public site values read beside it.
///
/// Every field is a path or a value the profile publishes to clients; none
/// is a secret, and the host builds it from the boot context without a
/// database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServicesTree {
    root: PathBuf,
    api_external_url: String,
    site_github_link: Option<String>,
}

impl ServicesTree {
    #[must_use]
    pub const fn new(
        root: PathBuf,
        api_external_url: String,
        site_github_link: Option<String>,
    ) -> Self {
        Self {
            root,
            api_external_url,
            site_github_link,
        }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn plugins(&self) -> PathBuf {
        self.root.join("plugins")
    }

    #[must_use]
    pub fn skills(&self) -> PathBuf {
        self.root.join("skills")
    }

    #[must_use]
    pub fn api_external_url(&self) -> &str {
        &self.api_external_url
    }

    #[must_use]
    pub fn site_github_link(&self) -> Option<&str> {
        self.site_github_link.as_deref()
    }
}

/// The session token of the person running the CLI.
///
/// The host resolves it through the same path the kernel's own commands use;
/// it is always the caller's own session and is never minted for another
/// subject.
#[derive(Debug, Clone)]
pub struct CallerSession {
    token: SessionToken,
}

impl CallerSession {
    #[must_use]
    pub const fn new(token: SessionToken) -> Self {
        Self { token }
    }

    #[must_use]
    pub const fn token(&self) -> &SessionToken {
        &self.token
    }
}
