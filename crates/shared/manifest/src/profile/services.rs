//! Profile-level overrides for the services manifest tree.
//!
//! An empty [`ServicesProfileConfig::sources`] means the instance serves the
//! services tree baked into the image at `paths.services`. Listing sources
//! makes the tree a composition of independently-published bundles, fetched at
//! boot in profile order.
//!
//! `deny_unknown_fields` rules out `serde(flatten)`, and `serde_yaml` renders
//! an externally-tagged enum as a `!Variant` YAML tag rather than a nested map,
//! so a source names its transport through optional `https:`/`oci:` sub-blocks
//! with exactly-one enforced in validation. The YAML reads:
//!
//! ```yaml
//! services:
//!   sources:
//!     - name: base
//!       oci:
//!         reference: ghcr.io/org/base@sha256:<64 hex>
//!         verify:
//!           ed25519_public_keys: ["<base64 32 bytes>"]
//!     - name: sales-uk
//!       https:
//!         url: https://example.test/bundle.tar.gz
//!         verify:
//!           sha256: "<64 hex>"
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServicesProfileConfig {
    #[serde(default)]
    pub port_offset: u16,

    #[serde(default)]
    pub sources: Vec<ServicesSource>,

    #[serde(default)]
    pub cache_dir: Option<String>,

    #[serde(default)]
    pub on_fetch_failure: FetchFailurePolicy,
}

impl ServicesProfileConfig {
    #[must_use]
    pub const fn is_identity(&self) -> bool {
        self.port_offset == 0 && self.sources.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServicesSource {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub https: Option<HttpsServicesSource>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oci: Option<OciServicesSource>,
}

impl ServicesSource {
    #[must_use]
    pub const fn verification(&self) -> Option<&BundleVerification> {
        if let Some(https) = self.https.as_ref() {
            return Some(&https.verify);
        }
        if let Some(oci) = self.oci.as_ref() {
            return Some(&oci.verify);
        }
        None
    }

    #[must_use]
    pub fn auth_secret(&self) -> Option<&str> {
        self.https
            .as_ref()
            .and_then(|s| s.auth_secret.as_deref())
            .or_else(|| self.oci.as_ref().and_then(|s| s.auth_secret.as_deref()))
    }

    #[must_use]
    pub const fn is_exactly_one(&self) -> bool {
        self.https.is_some() ^ self.oci.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HttpsServicesSource {
    pub url: String,

    #[serde(default)]
    pub auth_secret: Option<String>,

    #[serde(default)]
    pub verify: BundleVerification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OciServicesSource {
    pub reference: String,

    #[serde(default)]
    pub auth_secret: Option<String>,

    #[serde(default)]
    pub verify: BundleVerification,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleVerification {
    #[serde(default)]
    pub sha256: Option<String>,

    #[serde(default)]
    pub ed25519_public_keys: Vec<String>,
}

impl BundleVerification {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.sha256.is_none() && self.ed25519_public_keys.is_empty()
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum FetchFailurePolicy {
    FailClosed,

    #[default]
    UseLastGood,

    UseBundled,
}
