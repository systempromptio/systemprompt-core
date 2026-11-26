//! The report port a command reads, resolved from the composition.
//!
//! A port the composition does not provide is a typed
//! [`ReportUnavailable`] naming the report and the absent capability, never
//! an empty result that reads as zero usage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_extension::cli::CommandHost;
use systemprompt_traits::usage_reports::ReportUnavailable;

pub async fn report_port<T: Clone + Send + Sync + 'static>(
    host: &dyn CommandHost,
    report: &'static str,
    capability: &'static str,
) -> Result<T> {
    host.resolved()
        .await?
        .optional::<T>()
        .ok_or_else(|| ReportUnavailable { report, capability }.into())
}
