use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::audio::AudioArtifact;
use super::card::PresentationCardArtifact;
use super::chart::ChartArtifact;
use super::copy_paste_text::CopyPasteTextArtifact;
use super::dashboard::DashboardArtifact;
use super::image::ImageArtifact;
use super::list::ListArtifact;
use super::table::TableArtifact;
use super::text::TextArtifact;
use super::video::VideoArtifact;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Text,
    Table,
    Chart,
    Form,
    Dashboard,
    #[serde(rename = "presentation_card")]
    PresentationCard,
    List,
    #[serde(rename = "copy_paste_text")]
    CopyPasteText,
    Image,
    Video,
    Audio,
    #[serde(untagged)]
    Custom(String),
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "{}", TextArtifact::ARTIFACT_TYPE_STR),
            Self::Table => write!(f, "{}", TableArtifact::ARTIFACT_TYPE_STR),
            Self::Chart => write!(f, "{}", ChartArtifact::ARTIFACT_TYPE_STR),
            Self::Form => write!(f, "form"),
            Self::Dashboard => write!(f, "{}", DashboardArtifact::ARTIFACT_TYPE_STR),
            Self::PresentationCard => write!(f, "{}", PresentationCardArtifact::ARTIFACT_TYPE_STR),
            Self::List => write!(f, "{}", ListArtifact::ARTIFACT_TYPE_STR),
            Self::CopyPasteText => write!(f, "{}", CopyPasteTextArtifact::ARTIFACT_TYPE_STR),
            Self::Image => write!(f, "{}", ImageArtifact::ARTIFACT_TYPE_STR),
            Self::Video => write!(f, "{}", VideoArtifact::ARTIFACT_TYPE_STR),
            Self::Audio => write!(f, "{}", AudioArtifact::ARTIFACT_TYPE_STR),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    String,
    Integer,
    Number,
    Currency,
    Percentage,
    Date,
    Boolean,
    Link,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChartType {
    #[default]
    Line,
    Bar,
    Pie,
    Doughnut,
    Area,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AxisType {
    Category,
    #[default]
    Linear,
    Logarithmic,
    Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    Left,
    Center,
    Right,
}
