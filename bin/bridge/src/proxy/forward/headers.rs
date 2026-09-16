//! Header hygiene for proxied requests: hop-by-hop stripping, auth stamping,
//! and the client attestation the gateway records.
//!
//! The proxy is the only party that verified which host presented the
//! loopback credential, so it is the only party allowed to say so: an inbound
//! `x-systemprompt-client-attestation` is always dropped, and on the
//! inference route the proxy stamps its own. A per-host token names the host
//! outright (`host-token`) and overrides any `x-systemprompt-client` the
//! caller sent; the raw secret only proves the channel (`bridge-secret`), so
//! the caller's own declaration is passed through for the gateway to weigh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use hyper::HeaderMap;
use systemprompt_identifiers::{GatewayConversationId, SessionId, headers as sp_headers};
use systemprompt_models::wire::origin::{ClientAttestation, ClientKind};

use super::{ForwardError, ForwardResult};
use crate::proxy::credential::LoopbackCredential;

const HOP_BY_HOP: &[&str] = &[
    "host",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "content-length",
    "authorization",
    "x-api-key",
    "x-systemprompt-device-credential",
    "x-systemprompt-host",
    sp_headers::CLIENT_ATTESTATION,
    // Why: the OpenCode session rides upstream inside `metadata.user_id`,
    // never as a header the gateway would have to trust from any client.
    crate::feedback::sessions::OPENCODE_SESSION_HEADER,
];

#[derive(Debug, Clone, Copy)]
pub struct UpstreamHeaderInputs<'a> {
    pub src: &'a HeaderMap,
    pub bearer: &'a str,
    pub session_id: &'a SessionId,
    pub gateway_conversation_id: Option<&'a GatewayConversationId>,
    pub extra: &'a BTreeMap<String, String>,
    pub attest: Option<&'a LoopbackCredential>,
}

pub fn build_upstream_headers(inputs: &UpstreamHeaderInputs<'_>) -> ForwardResult<HeaderMap> {
    let UpstreamHeaderInputs {
        src,
        bearer,
        session_id,
        gateway_conversation_id,
        extra,
        attest,
    } = *inputs;
    let mut headers = HeaderMap::with_capacity(src.len() + 6 + extra.len());
    copy_request_headers(src, &mut headers);
    stamp_attestation(&mut headers, attest)?;

    let bearer = reqwest::header::HeaderValue::try_from(format!("Bearer {bearer}"))
        .map_err(|e| ForwardError::BadHeader(format!("authorization: {e}")))?;
    headers.insert(reqwest::header::AUTHORIZATION, bearer);
    let session_value = reqwest::header::HeaderValue::try_from(session_id.as_str())
        .map_err(|e| ForwardError::BadHeader(format!("{}: {e}", sp_headers::SESSION_ID)))?;
    headers.insert(
        reqwest::header::HeaderName::from_static(sp_headers::SESSION_ID),
        session_value,
    );
    if let Some(id) = gateway_conversation_id {
        let value = reqwest::header::HeaderValue::try_from(id.as_str()).map_err(|e| {
            ForwardError::BadHeader(format!("{}: {e}", sp_headers::GATEWAY_CONVERSATION_ID))
        })?;
        headers.insert(
            reqwest::header::HeaderName::from_static(sp_headers::GATEWAY_CONVERSATION_ID),
            value,
        );
    }

    for (k, v) in extra {
        let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| ForwardError::BadHeader(format!("{k}: {e}")))?;
        let value = reqwest::header::HeaderValue::try_from(v)
            .map_err(|e| ForwardError::BadHeader(format!("{k}: {e}")))?;
        headers.insert(name, value);
    }

    Ok(headers)
}

pub fn stamp_attestation(
    headers: &mut HeaderMap,
    attest: Option<&LoopbackCredential>,
) -> ForwardResult<()> {
    let client_kind = reqwest::header::HeaderName::from_static(sp_headers::CLIENT_KIND);
    let attestation = reqwest::header::HeaderName::from_static(sp_headers::CLIENT_ATTESTATION);
    let tier = match attest {
        None | Some(LoopbackCredential::Hook(_)) => {
            headers.remove(&client_kind);
            return Ok(());
        },
        Some(LoopbackCredential::Host(host)) => {
            let Some(kind) = ClientKind::from_bridge_host_id(host.as_str()) else {
                return Err(ForwardError::BadHeader(format!(
                    "{}: no client kind for host {host}",
                    sp_headers::CLIENT_KIND
                )));
            };
            headers.insert(
                client_kind,
                reqwest::header::HeaderValue::from_static(kind.as_str()),
            );
            ClientAttestation::HostToken
        },
        Some(LoopbackCredential::Secret) => ClientAttestation::BridgeSecret,
    };
    headers.insert(
        attestation,
        reqwest::header::HeaderValue::from_static(tier.as_str()),
    );
    Ok(())
}

pub fn copy_request_headers(src: &HeaderMap, dest: &mut HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.append(name, value);
    }
}

pub(super) fn copy_response_headers(src: &HeaderMap, dest: &mut HeaderMap) {
    for (name, value) in src {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        let (Ok(name), Ok(value)) = (
            hyper::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            hyper::header::HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        dest.insert(name, value);
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
}

pub(super) fn ensure_ingestion_delivery_id(
    request_path: &str,
    headers: &mut HeaderMap,
) -> ForwardResult<()> {
    // Why: retries of one inbound delivery must carry the same ingestion identity.
    if request_path.starts_with("/api/public/hooks/")
        && !headers.contains_key("x-ingestion-event-id")
    {
        let value = uuid::Uuid::new_v4()
            .to_string()
            .parse()
            .map_err(|error| ForwardError::BadHeader(format!("ingestion event ID: {error}")))?;
        headers.insert("x-ingestion-event-id", value);
    }

    Ok(())
}
