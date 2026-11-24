//! Integration tests for systemprompt-generator.
//!
//! These tests exercise the public surface end-to-end against on-disk
//! `dist/` scaffolds (BuildOrchestrator, validation, asset organisation)
//! and against in-memory provider mocks (RSS feed generation). DB-backed
//! prerender/sitemap entry points require a profile-installed `Config`
//! singleton and on-disk web/content YAML — they are not yet covered by
//! this crate; see the track report for the gap.

#[cfg(test)]
mod build_orchestrator_e2e;

#[cfg(test)]
mod validation_e2e;

#[cfg(test)]
mod rss_e2e;

#[cfg(test)]
mod assets_e2e;

#[cfg(test)]
mod error_e2e;

#[cfg(test)]
mod jobs_e2e;
