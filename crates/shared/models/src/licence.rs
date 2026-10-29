//! The licence check-in wire contract shared by the reporting job and the
//! control-plane receiver.
//!
//! A check-in is a JSON [`LicenceCheckin`] body sent by `POST` with
//! [`CHECKIN_SIGNATURE_HEADER`] set to `v1=<hex>`, where `<hex>` is the
//! lowercase hex HMAC-SHA256 of the exact body bytes keyed by the raw bytes
//! of the shared signing secret. [`CHECKIN_TENANT_HEADER`] carries the tenant
//! id when one is configured.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use systemprompt_identifiers::{EnterpriseLicenseId, InstanceId, TenantId};

pub const CHECKIN_SIGNATURE_HEADER: &str = "X-Systemprompt-Signature";
pub const CHECKIN_TENANT_HEADER: &str = "X-Systemprompt-Tenant";
pub const CHECKIN_SIGNATURE_VERSION: &str = "v1";

type HmacSha256 = Hmac<Sha256>;

/// The signed body of one licence check-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenceCheckin {
    pub licence_id: Option<EnterpriseLicenseId>,
    pub tenant_id: Option<TenantId>,
    pub instance_id: InstanceId,
    pub core_version: String,
    pub hosting_provider: String,
    pub external_host: Option<String>,
    pub instances_by_role: BTreeMap<String, i64>,
    pub timestamp: DateTime<Utc>,
}

/// Why a check-in signature header was rejected.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum CheckinSignatureError {
    #[error("signature header has no `v1=` version prefix")]
    UnknownVersion,
    #[error("signature is not valid hex: {0}")]
    Hex(#[source] hex::FromHexError),
    #[error("signature does not match the body")]
    Mismatch(#[source] hmac::digest::MacError),
}

fn mac(secret: &[u8]) -> HmacSha256 {
    #[expect(clippy::expect_used, reason = "HMAC-SHA256 accepts any key length")]
    <HmacSha256 as KeyInit>::new_from_slice(secret).expect("HMAC accepts any key length")
}

#[must_use]
pub fn sign_checkin(body: &[u8], secret: &[u8]) -> String {
    let mut mac = mac(secret);
    mac.update(body);
    format!(
        "{CHECKIN_SIGNATURE_VERSION}={}",
        hex::encode(mac.finalize().into_bytes())
    )
}

pub fn verify_checkin(
    body: &[u8],
    header_value: &str,
    secret: &[u8],
) -> Result<(), CheckinSignatureError> {
    let digest = header_value
        .strip_prefix(CHECKIN_SIGNATURE_VERSION)
        .and_then(|rest| rest.strip_prefix('='))
        .ok_or(CheckinSignatureError::UnknownVersion)?;
    let expected = hex::decode(digest).map_err(CheckinSignatureError::Hex)?;
    let mut mac = mac(secret);
    mac.update(body);
    mac.verify_slice(&expected)
        .map_err(CheckinSignatureError::Mismatch)
}
