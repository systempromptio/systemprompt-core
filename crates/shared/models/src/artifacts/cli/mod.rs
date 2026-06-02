//! CLI artifact envelope.
//!
//! [`CliArtifact`] is the tagged union of every renderable artifact a CLI
//! command can emit (table, list, text, dashboard, chart, media, card). The CLI
//! builds it, the wire carries it, and the MCP server deserializes it verbatim
//! — the `artifact_type` tag is intrinsic to the serde representation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    AudioArtifact, ChartArtifact, CopyPasteTextArtifact, DashboardArtifact, ImageArtifact,
    ListArtifact, PresentationCardArtifact, TableArtifact, TextArtifact, VideoArtifact,
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
}

impl CliArtifact {
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
            | Self::Video { .. } => None,
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
}
