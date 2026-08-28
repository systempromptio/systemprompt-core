//! Per-section data payloads for dashboard sections.
//!
//! Each struct here is the typed body of one dashboard section kind: metric
//! cards ([`MetricsCardsData`]/[`MetricCard`]), charts ([`ChartSectionData`]),
//! tables ([`TableSectionData`]), service/database status
//! ([`StatusSectionData`]), ranked lists ([`ListSectionData`]), timelines
//! ([`TimelineSectionData`]), and free text ([`TextSectionData`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod chart;
mod list;
mod metrics;
mod status;
mod table;
mod text;

pub use chart::ChartSectionData;
pub use list::{ItemList, ListItem, ListSectionData};
pub use metrics::{MetricCard, MetricStatus, MetricsCardsData};
pub use status::{DatabaseStatus, ErrorCounts, ServiceStatus, StatusSectionData};
pub use table::{SortConfig, TableSectionData};
pub use text::{TextSectionData, TimelineEvent, TimelineSectionData};
