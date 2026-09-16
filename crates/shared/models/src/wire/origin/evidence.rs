//! Everything the wire carried about the client, kept beside the
//! classification so it can be audited or re-derived.
//!
//! Every string is truncated to its `ai_request_client_evidence` column bound
//! at construction, so an insert can never trip the length CHECK.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::{ClientAttestation, ClientKind, NativeMarker};

pub(super) const DECLARED_CLIENT_MAX: usize = 64;
pub(super) const UA_PRODUCT_MAX: usize = 64;
pub(super) const UA_VERSION_MAX: usize = 64;
pub(super) const SDK_FIELD_MAX: usize = 64;

/// `kind_source` is the tier that named `client_kind`; on the `bridge-secret`
/// channel it is one of the lower tiers, since the secret says nothing about
/// the host. Nullable fields mean "not presented", never "empty".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientEvidence {
    pub kind_source: ClientAttestation,
    pub attested_host: Option<ClientKind>,
    pub declared_client: Option<String>,
    pub native_marker: Option<NativeMarker>,
    pub ua_product: Option<String>,
    pub ua_version: Option<String>,
    pub sdk_lang: Option<String>,
    pub sdk_package_version: Option<String>,
    pub sdk_runtime: Option<String>,
    pub sdk_runtime_version: Option<String>,
    pub sdk_os: Option<String>,
    pub sdk_arch: Option<String>,
}

impl ClientEvidence {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            kind_source: ClientAttestation::None,
            attested_host: None,
            declared_client: None,
            native_marker: None,
            ua_product: None,
            ua_version: None,
            sdk_lang: None,
            sdk_package_version: None,
            sdk_runtime: None,
            sdk_runtime_version: None,
            sdk_os: None,
            sdk_arch: None,
        }
    }

    #[must_use]
    pub const fn internal() -> Self {
        let mut evidence = Self::none();
        evidence.kind_source = ClientAttestation::Internal;
        evidence
    }
}

#[must_use]
pub(super) fn bounded(value: Option<&str>, max: usize) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    let mut end = value.len().min(max);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    Some(value[..end].to_owned())
}
