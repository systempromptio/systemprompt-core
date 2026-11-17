//! Serialisable output shapes for the `plugins` commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod capability_types;
mod extension_types;
mod seam_types;

pub use capability_types::{
    CapabilitiesSummaryOutput, CapabilitySummary, JobWithExtension, JobsListOutput,
    RoleWithExtension, RolesListOutput, SchemaWithExtension, SchemasListOutput,
};
pub use extension_types::{
    ExtensionConfigListOutput, ExtensionConfigOutput, ExtensionConfigSummary,
    ExtensionDetailOutput, ExtensionListOutput, ExtensionSource, ExtensionStanding,
    ExtensionSummary, JobInfo, RoleInfo, SchemaInfo,
};
pub use seam_types::SeamRow;
