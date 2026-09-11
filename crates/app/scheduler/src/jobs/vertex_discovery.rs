//! Daily re-probe of the Vertex AI publisher listing.
//!
//! The served catalog is fixed at boot — the provider registry lives behind a
//! `OnceLock` and is handed out as `&'static`, so nothing here can change what
//! this process serves. The job exists to *report*: it discovers against a
//! throwaway clone of the booted registry and warns when Vertex has published
//! a model the rate card already prices, which is the signal that a restart
//! would widen the catalog. It also reads the calendar: any served model whose
//! documented retirement or price change is within the notice window is
//! warned about, so an operator hears about it from the log before Google's
//! date arrives. The notice window is `LIFECYCLE_NOTICE_DAYS`;
//! `lifecycle_notices` yields `(model id, what changes, on which date)` for
//! every served rate-card model inside it, and `latest_report` exposes the
//! last report this process produced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{Days, NaiveDate};
use systemprompt_config::SecretsBootstrap;
use systemprompt_loader::ServicesBootstrap;
use systemprompt_models::services::{DiscoveryReport, ProviderRegistry, VertexRateCard};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::{info, warn};

static LATEST: RwLock<Option<DiscoveryReport>> = RwLock::new(None);

pub const LIFECYCLE_NOTICE_DAYS: u64 = 60;

#[must_use]
pub fn lifecycle_notices(
    served: &ProviderRegistry,
    card: &VertexRateCard,
    today: NaiveDate,
) -> Vec<(String, &'static str, NaiveDate)> {
    let horizon = today
        .checked_add_days(Days::new(LIFECYCLE_NOTICE_DAYS))
        .unwrap_or(today);
    let mut notices = Vec::new();
    for entry in &card.entries {
        let id = entry.id.as_str();
        if !served.contains_model(id) {
            continue;
        }
        if let Some(date) = entry.retires_on.filter(|d| *d <= horizon) {
            notices.push((id.to_owned(), "retires", date));
        }
        if let Some(date) = entry.price_until.filter(|d| *d <= horizon) {
            notices.push((id.to_owned(), "price changes after", date));
        }
    }
    notices
}

#[must_use]
pub fn latest_report() -> Option<DiscoveryReport> {
    LATEST
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[derive(Debug, Clone, Copy)]
pub struct VertexDiscoveryJob;

#[async_trait]
impl Job for VertexDiscoveryJob {
    fn name(&self) -> &'static str {
        "vertex_discovery"
    }

    fn description(&self) -> &'static str {
        "Re-probes Vertex AI for newly published models and reports the delta"
    }

    fn schedule(&self) -> &'static str {
        "0 30 4 * * *"
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();

        let Ok(booted) = ServicesBootstrap::providers() else {
            warn!("services config not initialized; skipping Vertex discovery");
            return Ok(JobResult::success().with_duration(start.elapsed().as_millis() as u64));
        };
        let Ok(secrets) = SecretsBootstrap::get() else {
            warn!("secret store unavailable; skipping Vertex discovery");
            return Ok(JobResult::success().with_duration(start.elapsed().as_millis() as u64));
        };

        // Why: a clone, never the live registry. Every reader holds a
        // `&'static` into the OnceLock, so mutating it in place would be
        // unsound as well as a silent mid-flight change of what is served.
        let mut probe: ProviderRegistry = booted.clone();
        let lookup = |name: &str| secrets.get(name).cloned();
        let report = systemprompt_loader::vertex_discovery::discover(
            &mut probe,
            &lookup,
            std::time::Duration::from_secs(30),
        )
        .await;

        if !report.discovered_priced.is_empty() {
            warn!(
                count = report.discovered_priced.len(),
                models = %report.discovered_priced.join(", "),
                "{} newly published Vertex models are priced and will be served after the next \
                 restart",
                report.discovered_priced.len()
            );
        }
        if !report.retiring.is_empty() {
            warn!(
                count = report.retiring.len(),
                models = %report.retiring.join(", "),
                "Vertex still lists {} models whose documentation has withdrawn or is retiring \
                 them; discovery withholds them",
                report.retiring.len()
            );
        }
        if let Ok(card) = VertexRateCard::embedded() {
            for (model, what, date) in
                lifecycle_notices(booted, &card, chrono::Utc::now().date_naive())
            {
                warn!(
                    model = %model,
                    date = %date,
                    "served Vertex model {model} {what} on {date}; plan its replacement"
                );
            }
        }
        if !report.failed_publishers.is_empty() {
            warn!(
                publishers = %report.failed_publishers.join(", "),
                "Vertex publisher listing failed; the report is partial"
            );
        }
        info!(
            priced = report.discovered_priced.len(),
            unpriced = report.discovered_unpriced.len(),
            "Vertex model discovery completed"
        );

        let scanned = report.discovered_priced.len() + report.discovered_unpriced.len();
        *LATEST
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(report);

        Ok(JobResult::success()
            .with_stats(scanned as u64, 0)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}

systemprompt_provider_contracts::submit_job!(&VertexDiscoveryJob);
