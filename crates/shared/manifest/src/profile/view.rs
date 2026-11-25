//! [`ProfileView`]: the non-secret profile facts an install member reads,
//! a host-sealed seed listed in
//! `systemprompt_extension::reserved::INSTALL_SEEDS`.
//!
//! The view is built once, at the composition root, from the loaded
//! [`Profile`]; it holds copies, never the profile. It carries the profile's
//! name, its target and runtime environment, whether the conversation judge
//! runs automatically, the cloud tenant id, the OTLP target and `paths.bin`.
//! The OTLP target is the endpoint without user info, query or fragment, its
//! protocol and its signals: the export headers, which carry the collector's
//! credentials, are not in it. The struct is exhaustive on purpose: a field
//! added to it is a contract amendment and a breaking change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TenantId;

use super::{Environment, OtlpExportConfig, OtlpProtocol, OtlpSignal, Profile, ProfileType};

/// The profile facts an install member may read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileView {
    pub name: String,
    pub target: ProfileType,
    pub environment: Environment,
    pub judge_automatic: bool,
    pub cloud_tenant_id: Option<TenantId>,
    pub otlp: Option<OtlpTargetView>,
    pub bin_path: String,
}

/// Where the OTLP exporter sends, without the credentials it sends with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpTargetView {
    pub endpoint: String,
    pub protocol: OtlpProtocol,
    pub signals: Vec<OtlpSignal>,
}

impl ProfileView {
    #[must_use]
    pub fn of(profile: &Profile) -> Self {
        Self {
            name: profile.name.clone(),
            target: profile.target,
            environment: profile.runtime.environment,
            judge_automatic: profile.judge.automatic,
            cloud_tenant_id: profile
                .cloud
                .as_ref()
                .and_then(|cloud| cloud.tenant_id.clone()),
            otlp: profile.observability.otlp().and_then(OtlpTargetView::of),
            bin_path: profile.paths.bin.clone(),
        }
    }
}

impl OtlpTargetView {
    fn of(config: &OtlpExportConfig) -> Option<Self> {
        Some(Self {
            endpoint: credential_free_url(&config.endpoint)?,
            protocol: config.protocol,
            signals: config.signals.clone(),
        })
    }
}

#[must_use]
pub fn credential_free_url(raw: &str) -> Option<String> {
    let mut url = url::Url::parse(raw).ok()?;
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    Some(url.into())
}
