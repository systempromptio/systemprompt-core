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
//! every served rate-card model inside it, and [`VertexDiscoveryJob::probe`]
//! returns the report one pass produced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::scheduling::job_app_context;
use async_trait::async_trait;
use chrono::{Days, NaiveDate};
use std::sync::Arc;
use systemprompt_manifest::secrets::Secrets;
use systemprompt_manifest::services::catalog::{CatalogDocument, ModelRegistry};
use systemprompt_manifest::services::{DiscoveryReport, ServicesView, VertexRateCard};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::{info, warn};

pub const LIFECYCLE_NOTICE_DAYS: u64 = 60;

#[must_use]
pub fn lifecycle_notices(
    served: &ModelRegistry,
    card: &VertexRateCard,
    today: NaiveDate,
) -> Vec<(String, &'static str, NaiveDate)> {
    let horizon = today
        .checked_add_days(Days::new(LIFECYCLE_NOTICE_DAYS))
        .unwrap_or(today);
    let mut notices = Vec::new();
    for entry in &card.entries {
        let id = entry.id.as_str();
        if served.get(id).is_none() {
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

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();
        let app = job_app_context(ctx)?;
        let services = app.services_state().view();
        let scanned = Self::probe(&services, app.secrets().get())
            .await
            .map_or(0, |report| {
                report.discovered_priced.len() + report.discovered_unpriced.len()
            });
        Ok(JobResult::success()
            .with_stats(scanned as u64, 0)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}

impl VertexDiscoveryJob {
    pub async fn probe(services: &ServicesView, secrets: Arc<Secrets>) -> Option<DiscoveryReport> {
        let booted = services.config();

        // Why: a clone, never the live catalog, so probing cannot change what
        // is served mid-flight.
        let mut probe = CatalogDocument {
            credentials: booted.credentials.clone(),
            providers: booted.providers.clone(),
            models: booted.models.clone(),
            deployments: booted.deployments.clone(),
            gateway: None,
        };
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
                "Newly published Vertex models are priced and will be served after the next \
                 restart"
            );
        }
        if !report.retiring.is_empty() {
            warn!(
                count = report.retiring.len(),
                models = %report.retiring.join(", "),
                "Vertex still lists models whose documentation has withdrawn or is retiring \
                 them; discovery withholds them"
            );
        }
        if let Ok(card) = VertexRateCard::embedded() {
            for (model, what, date) in
                lifecycle_notices(&booted.models, &card, chrono::Utc::now().date_naive())
            {
                warn!(
                    model = %model,
                    lifecycle = %what,
                    date = %date,
                    "Served Vertex model reaches a lifecycle deadline; plan its replacement"
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

        Some(report)
    }
}

systemprompt_provider_contracts::submit_job!(&VertexDiscoveryJob);
