//! Client attribution for one gateway request: the headers captured before
//! the body is consumed, and the classification that runs once the body and
//! the principal are known.
//!
//! The ladder itself is `systemprompt_models::wire::origin::classify`; this
//! module only feeds it and records the outcome on the rejection partial.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use systemprompt_identifiers::headers::{CLIENT_ATTESTATION, CLIENT_KIND};
use systemprompt_models::wire::origin::{
    ClassificationInput, ClientAttestation, ClientEvidence, ClientKind, StainlessHeaders, classify,
};

use super::RejectionPartial;

/// Copies of the attribution headers, taken before `read_gateway_body`
/// consumes the request. Values are decoded lossily so a non-ASCII
/// declaration reaches the classifier and is rejected there, not dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttributionHeaders {
    pub declared_client: Option<String>,
    pub declared_attestation: Option<String>,
    pub user_agent: Option<String>,
    pub sdk_lang: Option<String>,
    pub sdk_package_version: Option<String>,
    pub sdk_runtime: Option<String>,
    pub sdk_runtime_version: Option<String>,
    pub sdk_os: Option<String>,
    pub sdk_arch: Option<String>,
}

impl AttributionHeaders {
    #[must_use]
    pub fn capture(headers: &HeaderMap) -> Self {
        let lossy = |name: &str| {
            headers
                .get(name)
                .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
        };
        Self {
            declared_client: lossy(CLIENT_KIND),
            declared_attestation: lossy(CLIENT_ATTESTATION),
            user_agent: lossy(http::header::USER_AGENT.as_str()),
            sdk_lang: lossy("x-stainless-lang"),
            sdk_package_version: lossy("x-stainless-package-version"),
            sdk_runtime: lossy("x-stainless-runtime"),
            sdk_runtime_version: lossy("x-stainless-runtime-version"),
            sdk_os: lossy("x-stainless-os"),
            sdk_arch: lossy("x-stainless-arch"),
        }
    }

    #[must_use]
    pub fn stainless(&self) -> StainlessHeaders<'_> {
        StainlessHeaders {
            lang: self.sdk_lang.as_deref(),
            package_version: self.sdk_package_version.as_deref(),
            runtime: self.sdk_runtime.as_deref(),
            runtime_version: self.sdk_runtime_version.as_deref(),
            os: self.sdk_os.as_deref(),
            arch: self.sdk_arch.as_deref(),
        }
    }
}

// Why: a rejected declaration still leaves a row — `other`/`none` with the
// malformed value in the evidence — so the caller's mistake is auditable.
pub fn classify_client(
    attribution: &AttributionHeaders,
    principal_is_bridge: bool,
    body: &[u8],
    partial: &mut RejectionPartial,
) -> Result<ClientEvidence, (StatusCode, String)> {
    let input = ClassificationInput {
        principal_is_bridge,
        declared_client: attribution.declared_client.as_deref(),
        declared_attestation: attribution.declared_attestation.as_deref(),
        user_agent: attribution.user_agent.as_deref(),
        stainless: attribution.stainless(),
        body,
    };
    match classify(&input) {
        Ok(classified) => {
            if classified.conflicting {
                tracing::info!(
                    client_kind = classified.client.as_str(),
                    attestation = classified.attestation.as_str(),
                    declared = ?classified.evidence.declared_client,
                    native_marker = ?classified.evidence.native_marker,
                    ua_product = ?classified.evidence.ua_product,
                    "Client attribution signals disagree; strongest tier recorded"
                );
            }
            partial.origin.client = classified.client;
            partial.origin.attestation = classified.attestation;
            partial.evidence = Some(classified.evidence.clone());
            Ok(classified.evidence)
        },
        Err(rejection) => {
            partial.origin.client = ClientKind::Other;
            partial.origin.attestation = ClientAttestation::None;
            partial.evidence = Some(rejection.evidence().clone());
            Err((StatusCode::BAD_REQUEST, rejection.to_string()))
        },
    }
}

// Why: the entry-time origin is recorded before anything can reject, so the
// only evidence available is the User-Agent; the full ladder runs once the
// principal and body are known.
#[must_use]
pub fn entry_origin(headers: &HeaderMap) -> (ClientKind, ClientAttestation) {
    let user_agent = headers
        .get(http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let input = ClassificationInput {
        principal_is_bridge: false,
        declared_client: None,
        declared_attestation: None,
        user_agent,
        stainless: StainlessHeaders::default(),
        body: &[],
    };
    classify(&input).map_or((ClientKind::Other, ClientAttestation::None), |classified| {
        (classified.client, classified.attestation)
    })
}
