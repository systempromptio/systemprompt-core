pub mod audio;
pub mod card;
pub mod chart;
pub mod copy_paste_text;
pub mod dashboard;
pub mod image;
pub mod list;
pub mod metadata;
pub mod research;
pub mod table;
pub mod text;
pub mod traits;
pub mod types;
pub mod video;

pub use card::{CardCta, CardSection, PresentationCardArtifact, PresentationCardResponse};
pub use chart::{ChartArtifact, ChartDataset};
pub use copy_paste_text::CopyPasteTextArtifact;
pub use dashboard::{
    // Typed section data
    ChartSectionData,
    DashboardArtifact,
    DashboardHints,
    DashboardSection,
    DatabaseStatus,
    ErrorCounts,
    ItemList,
    LayoutMode,
    LayoutWidth,
    ListItem as DashboardListItem,
    ListSectionData,
    MetricCard,
    MetricStatus,
    MetricsCardsData,
    SectionLayout,
    SectionType,
    ServiceStatus,
    SortConfig,
    StatusSectionData,
    TableSectionData,
};
pub use list::{ListArtifact, ListItem};
pub use metadata::{ExecutionMetadata, ToolResponse};
pub use research::{ResearchArtifact, SourceCitation};
pub use table::{Column, TableArtifact, TableHints, TableResponse};
pub use text::TextArtifact;
pub use traits::{Artifact, ArtifactSchema};
pub use types::{Alignment, ArtifactType, AxisType, ChartType, ColumnType, SortOrder};

pub use audio::AudioArtifact;
pub use image::ImageArtifact;
pub use video::VideoArtifact;
