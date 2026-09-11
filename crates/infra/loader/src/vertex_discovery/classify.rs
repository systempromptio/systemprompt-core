//! Deciding which Vertex listing entries are models we can actually serve.
//!
//! A Model Garden listing mixes three populations under one JSON shape:
//! serverless models Google hosts and bills per token; *deployable
//! checkpoints*, which are weights plus a serving container and are callable
//! only after you stand up an endpoint yourself; and, under
//! `publishers/google`, every non-chat modality Google sells — embeddings,
//! speech, image, video, robotics. There is no field that separates them by
//! modality.
//!
//! So the rule is in two halves. Shape rules out what cannot be called at all
//! (a checkpoint with a `deploy` action; a partner entry that is not MaaS).
//! The rate card rules in what we are willing to serve — it is the only place
//! that knows `gemini-2.5-flash` is chat and `gemini-embedding-001` is not.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use systemprompt_models::services::{VertexRateCard, VertexRateCardEntry};

/// The MaaS suffix Vertex gives every serverless partner model.
const MAAS_SUFFIX: &str = "-maas";

const THIRD_PARTY_OSS: &str = "THIRD_PARTY_OWNED_OSS";

const GOOGLE_PUBLISHER: &str = "google";

const GA: &str = "GA";

/// One entry of `publisherModels`.
///
/// Unknown fields are ignored on purpose: the listing carries presentation
/// data (notebook links, container specs, regional availability) that grows
/// without notice, and a boot-time reader that fails on a new field would turn
/// a Google release note into an outage.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublisherModel {
    pub name: String,

    #[serde(default)]
    pub version_id: String,

    #[serde(default)]
    pub launch_stage: String,

    #[serde(default)]
    pub supported_actions: Option<serde_json::Value>,

    #[serde(default)]
    pub open_source_category: Option<String>,
}

impl PublisherModel {
    /// The publisher segment of `publishers/{publisher}/models/{model}`.
    #[must_use]
    pub fn publisher(&self) -> &str {
        let mut parts = self.name.split('/');
        if parts.next() == Some("publishers") {
            parts.next().unwrap_or_default()
        } else {
            ""
        }
    }

    /// The model segment of `publishers/{publisher}/models/{model}`.
    #[must_use]
    pub fn model_name(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }

    /// `{publisher}/{model}` — how the rate card names an upstream.
    #[must_use]
    pub fn upstream(&self) -> String {
        format!("{}/{}", self.publisher(), self.model_name())
    }

    #[must_use]
    pub fn is_generally_available(&self) -> bool {
        self.launch_stage == GA
    }

    fn is_deployable_checkpoint(&self) -> bool {
        self.supported_actions
            .as_ref()
            .is_some_and(|actions| actions.get("deploy").is_some())
    }

    fn has_no_actions(&self) -> bool {
        self.supported_actions.as_ref().is_none_or(|actions| {
            actions.as_object().is_some_and(serde_json::Map::is_empty) || actions.is_null()
        })
    }
}

/// Whether this entry can be called without deploying anything first.
///
/// Google's own publisher is served serverlessly across the board, so shape
/// says nothing there and everything is a candidate; the rate card does the
/// picking. A partner model qualifies only as MaaS: the `-maas` suffix, the
/// third-party OSS category, and no actions of its own.
#[must_use]
pub fn is_serverless(model: &PublisherModel) -> bool {
    if model.is_deployable_checkpoint() {
        return false;
    }
    if model.publisher() == GOOGLE_PUBLISHER {
        return true;
    }
    model.model_name().ends_with(MAAS_SUFFIX)
        && model.open_source_category.as_deref() == Some(THIRD_PARTY_OSS)
        && model.has_no_actions()
}

/// What discovery decided about one listing entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// Cannot be called without deploying it, or is not a MaaS partner model.
    NotServerless,
    /// Callable, but the rate card does not price it — so we will not serve it.
    Unpriced,
    /// Priced, but not GA and its card entry does not opt into previews.
    PreviewWithheld,
    /// Priced and publishable.
    Publish,
}

/// Classify one entry against the rate card of one provider.
#[must_use]
pub fn classify<'a>(
    model: &PublisherModel,
    card: &'a VertexRateCard,
    provider: &str,
) -> (Classification, Option<&'a VertexRateCardEntry>) {
    if !is_serverless(model) {
        return (Classification::NotServerless, None);
    }
    let upstream = model.upstream();
    let Some(entry) = card
        .lookup(&upstream)
        .filter(|e| e.provider.as_str() == provider)
    else {
        return (Classification::Unpriced, None);
    };
    if !model.is_generally_available() && !entry.allow_preview {
        return (Classification::PreviewWithheld, Some(entry));
    }
    (Classification::Publish, Some(entry))
}
