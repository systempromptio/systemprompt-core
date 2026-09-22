//! The evidence ladder: one pure function from what the wire carried to a
//! `client_kind`, its attestation tier, and the evidence row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::evidence::{
    DECLARED_CLIENT_MAX, SDK_FIELD_MAX, UA_PRODUCT_MAX, UA_VERSION_MAX, bounded,
};
use super::{ClientAttestation, ClientEvidence, ClientKind, NativeMarker};

/// The `x-stainless-*` headers every Stainless-generated SDK sends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StainlessHeaders<'a> {
    pub lang: Option<&'a str>,
    pub package_version: Option<&'a str>,
    pub runtime: Option<&'a str>,
    pub runtime_version: Option<&'a str>,
    pub os: Option<&'a str>,
    pub arch: Option<&'a str>,
}

/// Raw inputs to [`classify`]. `principal_is_bridge` is the only fact that
/// comes from authentication rather than the request itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassificationInput<'a> {
    pub principal_is_bridge: bool,
    pub declared_client: Option<&'a str>,
    pub declared_attestation: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub stainless: StainlessHeaders<'a>,
    pub body: &'a [u8],
}

/// `conflicting` is set when a weaker signal named a different client than
/// the winning tier; the caller logs it, nothing rejects it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub client: ClientKind,
    pub attestation: ClientAttestation,
    pub evidence: ClientEvidence,
    pub conflicting: bool,
}

/// A request the gateway refuses with `400`. The evidence gathered before the
/// rejection rides along so the rejected row still records what was sent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClassificationRejection {
    #[error("x-systemprompt-client must be one of {}", declarable_vocabulary())]
    MalformedDeclaredClient { evidence: Box<ClientEvidence> },
    #[error("x-systemprompt-client-attestation is set by the bridge only")]
    AttestationNotFromBridge { evidence: Box<ClientEvidence> },
    #[error("x-systemprompt-client-attestation must be host-token or bridge-secret")]
    MalformedAttestation { evidence: Box<ClientEvidence> },
    #[error("host-token attestation requires x-systemprompt-client")]
    HostTokenWithoutClient { evidence: Box<ClientEvidence> },
}

impl ClassificationRejection {
    #[must_use]
    pub const fn evidence(&self) -> &ClientEvidence {
        match self {
            Self::MalformedDeclaredClient { evidence }
            | Self::AttestationNotFromBridge { evidence }
            | Self::MalformedAttestation { evidence }
            | Self::HostTokenWithoutClient { evidence } => evidence,
        }
    }
}

fn declarable_vocabulary() -> String {
    ClientKind::DECLARABLE
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

enum Channel {
    HostToken,
    BridgeSecret,
}

fn initial_evidence(
    input: &ClassificationInput<'_>,
    marker: Option<NativeMarker>,
    ua: Option<&(String, Option<String>)>,
) -> ClientEvidence {
    ClientEvidence {
        kind_source: ClientAttestation::None,
        attested_host: None,
        declared_client: bounded(input.declared_client, DECLARED_CLIENT_MAX),
        native_marker: marker,
        ua_product: ua.and_then(|(p, _)| bounded(Some(p), UA_PRODUCT_MAX)),
        ua_version: ua.and_then(|(_, v)| bounded(v.as_deref(), UA_VERSION_MAX)),
        sdk_lang: bounded(input.stainless.lang, SDK_FIELD_MAX),
        sdk_package_version: bounded(input.stainless.package_version, SDK_FIELD_MAX),
        sdk_runtime: bounded(input.stainless.runtime, SDK_FIELD_MAX),
        sdk_runtime_version: bounded(input.stainless.runtime_version, SDK_FIELD_MAX),
        sdk_os: bounded(input.stainless.os, SDK_FIELD_MAX),
        sdk_arch: bounded(input.stainless.arch, SDK_FIELD_MAX),
    }
}

pub fn classify(input: &ClassificationInput<'_>) -> Result<Classified, ClassificationRejection> {
    let marker = native_marker(input.body);
    let ua = ua_product(input.user_agent);
    let ua_client = ua
        .as_ref()
        .and_then(|(product, _)| ClientKind::from_ua_product(product));
    let mut evidence = initial_evidence(input, marker, ua.as_ref());

    let channel = match (input.principal_is_bridge, input.declared_attestation) {
        (_, None) => None,
        (false, Some(_)) => {
            return Err(ClassificationRejection::AttestationNotFromBridge {
                evidence: Box::new(evidence),
            });
        },
        (true, Some(value)) => match ClientAttestation::parse(value) {
            Ok(ClientAttestation::HostToken) => Some(Channel::HostToken),
            Ok(ClientAttestation::BridgeSecret) => Some(Channel::BridgeSecret),
            _ => {
                return Err(ClassificationRejection::MalformedAttestation {
                    evidence: Box::new(evidence),
                });
            },
        },
    };

    let declared = match input.declared_client {
        None => None,
        Some(value) => match ClientKind::DECLARABLE
            .into_iter()
            .find(|kind| kind.as_str() == value)
        {
            Some(kind) => Some(kind),
            None => {
                return Err(ClassificationRejection::MalformedDeclaredClient {
                    evidence: Box::new(evidence),
                });
            },
        },
    };

    let (client, source) = match channel {
        Some(Channel::HostToken) => {
            let Some(host) = declared else {
                return Err(ClassificationRejection::HostTokenWithoutClient {
                    evidence: Box::new(evidence),
                });
            };
            evidence.attested_host = Some(host);
            (host, ClientAttestation::HostToken)
        },
        _ => declared
            .map(|kind| (kind, ClientAttestation::Declared))
            .or_else(|| marker.map(|m| (m.client(), ClientAttestation::NativeMarker)))
            .or_else(|| ua_client.map(|kind| (kind, ClientAttestation::UserAgent)))
            .unwrap_or((ClientKind::Other, ClientAttestation::None)),
    };
    evidence.kind_source = source;
    let attestation = match channel {
        Some(Channel::BridgeSecret) => ClientAttestation::BridgeSecret,
        _ => source,
    };
    let conflicting = [declared, marker.map(NativeMarker::client), ua_client]
        .into_iter()
        .flatten()
        .any(|named| !named.same_runtime(client));

    Ok(Classified {
        client,
        attestation,
        evidence,
        conflicting,
    })
}

// JSON: protocol boundary — the inference body is any host's wire shape.
#[must_use]
pub fn native_marker(body: &[u8]) -> Option<NativeMarker> {
    if body.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if value
        .pointer("/client_metadata/x-codex-turn-metadata")
        .is_some()
    {
        return Some(NativeMarker::CodexTurnMetadata);
    }
    claude_entrypoint(&value).or_else(|| claude_metadata_user_id(&value))
}

// Why: Claude Code puts the billing header in the first system block on a
// third-party gateway; a Cowork session runs the same runtime as the CLI and
// this prefix is the one place the body says which of the two it is.
fn claude_entrypoint(value: &serde_json::Value) -> Option<NativeMarker> {
    let system = value.get("system")?;
    let text = match system {
        serde_json::Value::String(text) => text.as_str(),
        serde_json::Value::Array(blocks) => blocks.first()?.get("text")?.as_str()?,
        _ => return None,
    };
    let rest = text
        .trim_start()
        .strip_prefix("x-anthropic-billing-header:")?;
    let entrypoint = rest
        .split(';')
        .map(str::trim)
        .find_map(|field| field.strip_prefix("cc_entrypoint="))?;
    match entrypoint {
        "cli" => Some(NativeMarker::ClaudeCliEntrypoint),
        "local-agent" => Some(NativeMarker::ClaudeDesktopEntrypoint),
        entry if entry.starts_with("claude-desktop") => Some(NativeMarker::ClaudeDesktopEntrypoint),
        _ => None,
    }
}

fn claude_metadata_user_id(value: &serde_json::Value) -> Option<NativeMarker> {
    let user_id = value.pointer("/metadata/user_id")?.as_str()?.trim();
    if user_id.starts_with('{') {
        // JSON: protocol boundary — Claude Code ≥ 2.1.25x stamps its session
        // as a JSON string; `device_id` beside `session_id` is the shape.
        let metadata: serde_json::Value = serde_json::from_str(user_id).ok()?;
        return (metadata.get("device_id").is_some_and(serde_json::Value::is_string)
            && metadata.get("session_id").is_some_and(serde_json::Value::is_string))
        .then_some(NativeMarker::ClaudeMetadataJson);
    }
    // Why: `user_<hex>_account_<uuid>_session_<uuid>` is the grammar Claude
    // Code and Claude Desktop stamp; the checks are structural, not a match on
    // any client-chosen text.
    let mut parts = user_id.split('_');
    let shape = [
        parts.next() == Some("user"),
        parts.next().is_some_and(|hex| !hex.is_empty()),
        parts.next() == Some("account"),
        parts
            .next()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()),
        parts.next() == Some("session"),
        parts
            .next()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()),
        parts.next().is_none(),
    ];
    shape
        .iter()
        .all(|ok| *ok)
        .then_some(NativeMarker::ClaudeMetadataUserId)
}

// Why: the first product token is the only part of a User-Agent with a
// defined grammar (RFC 9110 §10.1.5); everything after it is free text and is
// never matched.
#[must_use]
pub fn ua_product(user_agent: Option<&str>) -> Option<(String, Option<String>)> {
    let first = user_agent?.split_ascii_whitespace().next()?;
    let (product, version) = first
        .split_once('/')
        .map_or((first, None), |(p, v)| (p, Some(v)));
    if product.is_empty() || !product.chars().all(is_token_char) {
        return None;
    }
    Some((
        product.to_ascii_lowercase(),
        version.filter(|v| !v.is_empty()).map(str::to_owned),
    ))
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)
}
