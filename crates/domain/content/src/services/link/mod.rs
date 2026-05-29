//! Link services.
//!
//! Pairs link creation with click analytics: [`LinkGenerationService`] mints
//! trackable campaign links, and `LinkAnalyticsService` reports on the clicks
//! and conversions those links accrue.

pub mod analytics;
pub mod generation;

pub use analytics::LinkAnalyticsService;
pub use generation::{GenerateLinkParams, LinkGenerationService};
