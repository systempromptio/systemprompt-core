//! The evidence ladder: one pure function from what the wire carried to a
//! `client_kind`, its attestation tier, and the evidence row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::evidence::{
    DECLARED_CLIENT_MAX, SDK_FIELD_MAX, UA_PRODUCT_MAX, UA_VERSION_MAX, bounded,
};
use super::signals::{native_marker, ua_product};
use super::{ClientAttestation, ClientEvidence, ClientKind, NativeMarker};
use crate::bridge::host::ConfigOwner;

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
    #[error("host-token attestation is not issued for a client that configures itself")]
    HostTokenForHostOwnedClient { evidence: Box<ClientEvidence> },
    #[error("host-token attestation requires a client the bridge configures a host for")]
    HostTokenForClientWithoutHost { evidence: Box<ClientEvidence> },
}

impl ClassificationRejection {
    #[must_use]
    pub const fn evidence(&self) -> &ClientEvidence {
        match self {
            Self::MalformedDeclaredClient { evidence }
            | Self::AttestationNotFromBridge { evidence }
            | Self::MalformedAttestation { evidence }
            | Self::HostTokenWithoutClient { evidence }
            | Self::HostTokenForHostOwnedClient { evidence }
            | Self::HostTokenForClientWithoutHost { evidence } => evidence,
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

fn attest_host(
    declared: Option<ClientKind>,
    mut evidence: ClientEvidence,
) -> Result<(ClientKind, ClientEvidence), ClassificationRejection> {
    let Some(host) = declared else {
        return Err(ClassificationRejection::HostTokenWithoutClient {
            evidence: Box::new(evidence),
        });
    };
    let mut packagings = host.hosts().peekable();
    if packagings.peek().is_none() {
        return Err(ClassificationRejection::HostTokenForClientWithoutHost {
            evidence: Box::new(evidence),
        });
    }
    if packagings.any(|packaging| packaging.config_owner() == ConfigOwner::Host) {
        return Err(ClassificationRejection::HostTokenForHostOwnedClient {
            evidence: Box::new(evidence),
        });
    }
    evidence.attested_host = Some(host);
    Ok((host, evidence))
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
            let (host, attested) = attest_host(declared, evidence)?;
            evidence = attested;
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
