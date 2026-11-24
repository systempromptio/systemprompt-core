//! Rejection reasons for an external marketplace or pass-through plugin entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

use systemprompt_models::managed::RevisionBundleError;
use systemprompt_models::net::OutboundUrlError;

pub(super) const MAX_GIT_REF_CHARS: usize = 128;

#[derive(Debug, Error)]
pub(super) enum ExternalEntryError {
    #[error("`ref` must not be empty")]
    EmptyRef,

    #[error("`ref` must be at most {MAX_GIT_REF_CHARS} characters")]
    RefTooLong,

    #[error("`ref` {reference:?} must not contain whitespace")]
    RefWhitespace { reference: String },

    #[error("`ref` {reference:?} must not contain '..'")]
    RefParentTraversal { reference: String },

    #[error("`ref` {reference:?} must not start with '-'")]
    RefLeadingDash { reference: String },

    #[error("must be named with letters, digits, '-', '_' and '.' only")]
    InvalidName,

    #[error("github repo '{repo}' must be 'owner/repository'")]
    GithubRepo { repo: String },

    #[error("path '{path}' must be a relative path: {source}")]
    Path {
        path: String,
        #[source]
        source: RevisionBundleError,
    },

    #[error("url '{url}' is not a usable public URL: {source}")]
    UnusableUrl {
        url: String,
        #[source]
        source: OutboundUrlError,
    },

    #[error("url '{url}' must use https")]
    NotHttps { url: String },

    #[error(
        "`sha` '{sha}' must be a full lowercase commit id — a pass-through plugin is never \
         inspected, so it must be pinned"
    )]
    UnpinnedSha { sha: String },
}
