use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    /// Custom artifact types defined by extensions (e.g., "blog", "product",
    /// etc.)
    #[serde(untagged)]
    Custom(String),
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Table => write!(f, "table"),
            Self::Chart => write!(f, "chart"),
            Self::Form => write!(f, "form"),
            Self::Dashboard => write!(f, "dashboard"),
            Self::PresentationCard => write!(f, "presentation_card"),
            Self::List => write!(f, "list"),
            Self::CopyPasteText => write!(f, "copy_paste_text"),
            Self::Image => write!(f, "image"),
            Self::Video => write!(f, "video"),
            Self::Audio => write!(f, "audio"),
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
