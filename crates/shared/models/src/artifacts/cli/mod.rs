//! CLI artifact envelope.
//!
//! [`CliArtifact`] is the tagged union of every renderable artifact a CLI
//! command can emit (table, list, text, dashboard, chart, media, card,
//! message). The CLI builds it, the wire carries it, and the MCP server
//! deserializes it verbatim — the `artifact_type` tag is intrinsic to the
//! serde representation.
//!
//! # Wire contract
//!
//! Two distinct tags travel with an enveloped artifact, and they are
//! deliberately NOT unified:
//!
//! - The **envelope tag** [`CliArtifact::ENVELOPE_TYPE_STR`] (`"cli"`) is
//!   advertised in tool output schemas (the top-level `x-artifact-type`). It
//!   says "this output is a `CliArtifact` union", never which variant.
//! - The **variant tag** is embedded in the serialized data itself: the
//!   `artifact_type` serde tag (e.g. `"table"`), mirrored by the inner
//!   artifact's `x-artifact-type` field.
//!
//! Schema consumers route on the envelope tag; renderers and type inference
//! must fall through it to the data-embedded variant tag. Collapsing the two
//! would either erase the union from the schema or mis-type every enveloped
//! artifact, so both tags stay on the wire.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    AudioArtifact, ChartArtifact, CopyPasteTextArtifact, DashboardArtifact, ImageArtifact,
    ListArtifact, MessageArtifact, PresentationCardArtifact, TableArtifact, TextArtifact,
    VideoArtifact,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "artifact_type", rename_all = "snake_case")]
pub enum CliArtifact {
    Table {
        #[serde(flatten)]
        artifact: TableArtifact,
    },
    List {
        #[serde(flatten)]
        artifact: ListArtifact,
    },
    Text {
        #[serde(flatten)]
        artifact: TextArtifact,
    },
    #[serde(rename = "copy_paste_text")]
    CopyPasteText {
        #[serde(flatten)]
        artifact: CopyPasteTextArtifact,
    },
    Dashboard {
        #[serde(flatten)]
        artifact: DashboardArtifact,
    },
    Chart {
        #[serde(flatten)]
        artifact: ChartArtifact,
    },
    Audio {
        #[serde(flatten)]
        artifact: AudioArtifact,
    },
    Image {
        #[serde(flatten)]
        artifact: ImageArtifact,
    },
    Video {
        #[serde(flatten)]
        artifact: VideoArtifact,
    },
    #[serde(rename = "presentation_card")]
    PresentationCard {
        #[serde(flatten)]
        artifact: PresentationCardArtifact,
    },
    Message {
        #[serde(flatten)]
        artifact: MessageArtifact,
    },
}

impl CliArtifact {
    pub const ENVELOPE_TYPE_STR: &'static str = "cli";

    #[must_use]
    pub const fn artifact_type_str(&self) -> &'static str {
        match self {
            Self::Table { .. } => TableArtifact::ARTIFACT_TYPE_STR,
            Self::List { .. } => ListArtifact::ARTIFACT_TYPE_STR,
            Self::Text { .. } => TextArtifact::ARTIFACT_TYPE_STR,
            Self::CopyPasteText { .. } => CopyPasteTextArtifact::ARTIFACT_TYPE_STR,
            Self::Dashboard { .. } => DashboardArtifact::ARTIFACT_TYPE_STR,
            Self::Chart { .. } => ChartArtifact::ARTIFACT_TYPE_STR,
            Self::Audio { .. } => AudioArtifact::ARTIFACT_TYPE_STR,
            Self::Image { .. } => ImageArtifact::ARTIFACT_TYPE_STR,
            Self::Video { .. } => VideoArtifact::ARTIFACT_TYPE_STR,
            Self::PresentationCard { .. } => PresentationCardArtifact::ARTIFACT_TYPE_STR,
            Self::Message { .. } => MessageArtifact::ARTIFACT_TYPE_STR,
        }
    }

    #[must_use]
    pub fn title(&self) -> Option<String> {
        match self {
            Self::Text { artifact } => artifact.title.clone(),
            Self::CopyPasteText { artifact } => artifact.title.clone(),
            Self::Dashboard { artifact } => Some(artifact.title.clone()),
            Self::Audio { artifact } => artifact.title.clone(),
            Self::PresentationCard { artifact } => Some(artifact.title.clone()),
            Self::Table { .. }
            | Self::List { .. }
            | Self::Chart { .. }
            | Self::Image { .. }
            | Self::Video { .. }
            | Self::Message { .. } => None,
        }
    }

    #[must_use]
    pub const fn table(artifact: TableArtifact) -> Self {
        Self::Table { artifact }
    }

    #[must_use]
    pub const fn list(artifact: ListArtifact) -> Self {
        Self::List { artifact }
    }

    #[must_use]
    pub const fn text(artifact: TextArtifact) -> Self {
        Self::Text { artifact }
    }

    #[must_use]
    pub const fn copy_paste_text(artifact: CopyPasteTextArtifact) -> Self {
        Self::CopyPasteText { artifact }
    }

    #[must_use]
    pub const fn dashboard(artifact: DashboardArtifact) -> Self {
        Self::Dashboard { artifact }
    }

    #[must_use]
    pub const fn chart(artifact: ChartArtifact) -> Self {
        Self::Chart { artifact }
    }

    #[must_use]
    pub const fn audio(artifact: AudioArtifact) -> Self {
        Self::Audio { artifact }
    }

    #[must_use]
    pub const fn image(artifact: ImageArtifact) -> Self {
        Self::Image { artifact }
    }

    #[must_use]
    pub const fn video(artifact: VideoArtifact) -> Self {
        Self::Video { artifact }
    }

    #[must_use]
    pub const fn presentation_card(artifact: PresentationCardArtifact) -> Self {
        Self::PresentationCard { artifact }
    }

    #[must_use]
    pub const fn message(artifact: MessageArtifact) -> Self {
        Self::Message { artifact }
    }
}
