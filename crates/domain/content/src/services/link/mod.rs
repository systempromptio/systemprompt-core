//! Link services.
//!
//! Pairs link creation with click analytics: [`LinkGenerationService`] mints
//! trackable campaign links, and `LinkAnalyticsService` reports on the clicks
//! and conversions those links accrue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod analytics;
pub mod generation;

pub use analytics::LinkAnalyticsService;
pub use generation::{GenerateLinkParams, LinkGenerationService};
