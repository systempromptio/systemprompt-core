//! Registers analytics-owned schemas, rollups, report views and migrations.
//!
//! That covers the engagement and reputation schemas, the hourly tool and
//! agent-task rollups, and the report views over source tables. It depends on
//! `agent` (and through it `ai` and `mcp`) because the rollup triggers sit on
//! their tables.
//!
//! On every host it provides the content audience ranking
//! ([`DynContentPopularity`]) the
//! content prerender orders its popular lists by.
//!
//! It provides its repositories (`Arc<AnalyticsRepositories>`), built from the
//! kernel's database pool, the session store the `users` member provides and
//! the logging event store; the `Arc<AnalyticsService>` over them, enriched
//! with the `GeoIP` reader and the content routing when the kernel seeds
//! them; and the
//! [`DynSessionBehaviourScorer`]
//! the request-analytics middleware hands each tracked request's session to.
//!
//! It contributes the analytics jobs (session classification, the on-demand
//! geo backfill, the two detectors that feed the IP ban list, and the
//! link-click projection drainer) and the retention purges of its tables.
//!
//! The reference composition marks the member required: it owns the
//! protective jobs (the malicious-IP blacklist and the behavioural bot
//! analysis that feeds throttling and bans), so a profile that disables it
//! is refused at composition rather than booting with those controls off.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_traits::{
    DynContentPopularity, DynIpBanStore, DynSessionBehaviourScorer, DynSessionStore,
};

use systemprompt_logging::AnalyticsRepository;
use systemprompt_models::{ContentConfigRaw, ContentRouting};

use crate::jobs::{
    BackfillSessionGeoJob, BehavioralAnalysisJob, EngagementEventsPurge,
    FingerprintReputationPurge, GhostSessionCleanupJob, LinkFactsRollupJob,
    MaliciousIpBlacklistJob, NoJsCleanupJob,
};
use crate::repository::{AnalyticsRepositories, ContentAnalyticsRepository};
use crate::{AnalyticsService, GeoIpReader, SessionBehaviourService};

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalyticsExtension;

impl Extension for AnalyticsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "analytics",
            name: "Analytics",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in schemas() {
            c.schema(schema);
        }
        c.migrations(extension_migrations!())
            .requires::<DynIpBanStore>()
            .provide(|r: &Resolved<'_>| {
                let db = r.get::<DbPool>()?;
                let popularity: DynContentPopularity =
                    Arc::new(ContentAnalyticsRepository::new(&db));
                Ok(popularity)
            })
            .provide(|r: &Resolved<'_>| {
                let db = r.get::<DbPool>()?;
                let sessions = r.get::<DynSessionStore>()?;
                let event_log = Arc::new(AnalyticsRepository::new(&db));
                Ok(Arc::new(AnalyticsRepositories::new(
                    &db, sessions, event_log,
                )))
            })
            .provide(|r: &Resolved<'_>| {
                let repositories = r.get::<Arc<AnalyticsRepositories>>()?;
                let routing = r.optional::<Arc<ContentConfigRaw>>().map(|config| {
                    let routing: Arc<dyn ContentRouting> = config;
                    routing
                });
                Ok(Arc::new(AnalyticsService::new(
                    r.optional::<GeoIpReader>(),
                    routing,
                    &repositories,
                )))
            })
            .provide(|r: &Resolved<'_>| {
                let repositories = r.get::<Arc<AnalyticsRepositories>>()?;
                let sessions = r.get::<DynSessionStore>()?;
                let scorer: DynSessionBehaviourScorer =
                    SessionBehaviourService::new(sessions, repositories.session_signals.clone())
                        .into_scorer();
                Ok(scorer)
            })
            .job(GhostSessionCleanupJob)
            .job(NoJsCleanupJob)
            .job(BackfillSessionGeoJob)
            .job(BehavioralAnalysisJob)
            .job(MaliciousIpBlacklistJob)
            .job(LinkFactsRollupJob)
            .retention(RetentionDecl::Purge(Arc::new(EngagementEventsPurge)))
            .retention(RetentionDecl::Purge(Arc::new(FingerprintReputationPurge)));
        c.owner_reassignment(
            |r: &Resolved<'_>| -> Result<systemprompt_traits::DynOwnerReassignment, FactoryError> {
                Ok(Arc::new(
                    crate::repository::AnalyticsOwnerReassignment::new(&r.get::<DbPool>()?),
                ))
            },
        );

        crate::jobs::rollups::contribute(c);
        Ok(())
    }
}

fn schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "engagement_events",
            include_str!("../schema/engagement_events.sql"),
        )
        .with_required_columns(vec!["id".into(), "session_id".into(), "created_at".into()]),
        SchemaDefinition::new(
            "fingerprint_reputation",
            include_str!("../schema/fingerprint_reputation.sql"),
        )
        .with_required_columns(vec!["fingerprint_hash".into()]),
        SchemaDefinition::new(
            "tool_usage_rollup_hourly",
            include_str!("../schema/usage_rollups.sql"),
        )
        .with_required_columns(vec!["bucket".into(), "execution_count".into()]),
        SchemaDefinition::new(
            "agent_task_rollup_hourly",
            include_str!("../schema/agent_task_rollup_hourly.sql"),
        )
        .with_required_columns(vec!["bucket".into(), "task_count".into()]),
        SchemaDefinition::sql_only(include_str!("../schema/report_views.sql")),
    ]
}
