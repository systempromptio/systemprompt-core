//! The HTTP pieces of the document store shared by every route set that
//! serves it.
//!
//! They are the one mapping of document failures onto the error envelope
//! ([`error`]), the conditional and ranged request headers ([`headers`]),
//! the request body ceiling a document route admits, the internal link route
//! ([`internal_links`]) and the [`kernel::DocumentsKernelService`] the
//! `documents` member provides for the API. The `FilesSurface` contract the
//! `files-http` member fulfils lives in `systemprompt-extension`, so the API
//! mounts it without naming this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "collab")]
mod collab_error;
pub mod error;
pub mod headers;
pub mod internal_links;
pub mod kernel;

use systemprompt_files::documents::DocumentService;

const BODY_SLACK_BYTES: u64 = 64 * 1024;

#[must_use]
pub fn body_limit(service: &DocumentService) -> usize {
    let limit = service
        .config()
        .max_upload_bytes
        .saturating_add(BODY_SLACK_BYTES);
    usize::try_from(limit).unwrap_or(usize::MAX)
}
