//! The stored verified envelope as a revalidation cache for manifest fetches.
//!
//! The envelope a sync accepted is kept on disk with the gateway's ETag; the
//! next fetch sends it back as `If-None-Match`, and a 304 resolves to that
//! stored envelope instead of a multi-megabyte download. The stored envelope is
//! still signature-verified by the caller exactly as a fresh one would be.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::error::SyncError;
use crate::gateway::manifest::SignedManifestEnvelope;
use crate::gateway::{GatewayError, ManifestResponse};
use crate::mcp_registry::EnvelopeFragment;

pub(super) fn stored_envelope(gateway: &ValidatedUrl) -> Option<EnvelopeFragment> {
    match crate::mcp_registry::read_envelope() {
        Ok(Some(fragment)) if crate::mcp_registry::same_origin(&fragment.gateway, gateway) => {
            Some(fragment)
        },
        Ok(_) => None,
        Err(error) => {
            tracing::debug!(%error, "stored manifest envelope unreadable; fetching in full");
            None
        },
    }
}

pub(super) fn resolve(
    response: ManifestResponse,
    stored: Option<EnvelopeFragment>,
) -> Result<(SignedManifestEnvelope, Option<String>, bool), SyncError> {
    match (response, stored) {
        (ManifestResponse::Fresh { envelope, etag }, _) => Ok((envelope, etag, false)),
        (ManifestResponse::NotModified, Some(fragment)) => {
            tracing::debug!(
                target: "bridge::sync",
                "manifest not modified; reusing the stored verified envelope"
            );
            Ok((fragment.envelope, fragment.etag, true))
        },
        (ManifestResponse::NotModified, None) => Err(SyncError::Gateway(
            GatewayError::ManifestNotModifiedWithoutCache,
        )),
    }
}
