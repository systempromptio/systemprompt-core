//! Chart section payload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::artifacts::chart::ChartDataset;
use crate::artifacts::types::AxisType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChartSectionData {
    pub chart_type: String,
    pub labels: Vec<String>,
    pub datasets: Vec<ChartDataset>,
    #[serde(default)]
    pub x_axis_label: String,
    #[serde(default)]
    pub y_axis_label: String,
    #[serde(default)]
    pub y_axis_type: AxisType,
}

impl ChartSectionData {
    pub fn new(
        chart_type: impl Into<String>,
        labels: Vec<String>,
        datasets: Vec<ChartDataset>,
    ) -> Self {
        Self {
            chart_type: chart_type.into(),
            labels,
            datasets,
            x_axis_label: String::new(),
            y_axis_label: String::new(),
            y_axis_type: AxisType::default(),
        }
    }

    #[must_use]
    pub fn with_axis_labels(
        mut self,
        x_axis_label: impl Into<String>,
        y_axis_label: impl Into<String>,
    ) -> Self {
        self.x_axis_label = x_axis_label.into();
        self.y_axis_label = y_axis_label.into();
        self
    }

    #[must_use]
    pub const fn with_y_axis_type(mut self, y_axis_type: AxisType) -> Self {
        self.y_axis_type = y_axis_type;
        self
    }
}
