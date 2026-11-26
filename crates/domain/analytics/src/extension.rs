//! Registers analytics-owned schemas, report views and migrations.
//!
//! That covers the engagement and reputation schemas and the report views
//! over tables the kernel always links. Its schema names no table of an
//! optional member: the hourly tool and agent-task rollups and the report
//! views over the `ai`, `agent`, `mcp` and `content` tables are contributed
//! by the member owning those tables, so the member installs with the kernel
//! alone.
//!
//! It provides the per-endpoint viewer counts (`DynContentAudienceCounts`)
//! the content member ranks its popular lists by.
//!
//! It provides its repositories (`Arc<AnalyticsRepositories>`), built from the
//! kernel's database pool, the session store the `users` member provides and
//! the logging event store; the `GeoIP` database it opens from the
//! configured path (`Arc<GeoIpDatabase>`, empty when none is configured);
//! the `Arc<AnalyticsService>` over them, enriched with that database and the
//! content routing when the kernel seeds it, and the same service as the open
//! `Arc<dyn AnalyticsProvider>`; the open `Arc<dyn FingerprintProvider>` over
//! the fingerprint repository; and the
//! [`DynSessionBehaviourScorer`]
//! the request-analytics middleware hands each tracked request's session to,
//! and the open request-analytics write (`DynRequestAnalytics`) that
//! middleware records each tracked request through.
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

use crate::repository::ContentAudienceRepository;
use systemprompt_traits::usage_reports::content::DynContentAudienceCounts;
use systemprompt_traits::{DynIpBanStore, DynSessionBehaviourScorer, DynSessionTelemetry};

use systemprompt_logging::AnalyticsRepository;
use systemprompt_models::{ContentConfigRaw, ContentRouting};

use crate::jobs::{
    BackfillSessionGeoJob, BehavioralAnalysisJob, EngagementEventsPurge,
    FingerprintReputationPurge, GhostSessionCleanupJob, LinkFactsRollupJob,
    MaliciousIpBlacklistJob, NoJsCleanupJob,
};
use crate::repository::AnalyticsRepositories;
use crate::{AnalyticsService, GeoIpDatabase, SessionBehaviourService};

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
            .provide(crate::provides::request_analytics)
            .provide(crate::provides::geoip_database)
            .provide(|r: &Resolved<'_>| {
                let db = r.get::<DbPool>()?;
                let counts: DynContentAudienceCounts =
                    Arc::new(ContentAudienceRepository::new(&db));
                Ok(counts)
            })
            .provide(|r: &Resolved<'_>| {
                let db = r.get::<DbPool>()?;
                let sessions = r.get::<DynSessionTelemetry>()?;
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
                    r.get::<Arc<GeoIpDatabase>>()?.reader().cloned(),
                    routing,
                    &repositories,
                )))
            })
            .provide(crate::provides::analytics_provider)
            .provide(crate::provides::fingerprint_provider)
            .provide(|r: &Resolved<'_>| {
                let repositories = r.get::<Arc<AnalyticsRepositories>>()?;
                let sessions = r.get::<DynSessionTelemetry>()?;
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
        c.owner_reassignment(OwnerReassignment {
            table: "engagement_events",
            column: "user_id",
            action: ReassignAction::UPDATE,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "fingerprint_reputation",
            column: "associated_user_ids",
            action: ReassignAction::ReplaceInArray,
        });

        c.retire(SchemaDefinition {
            table: None,
            sql: include_str!("../schema/retirements/retired_tables.sql").to_owned(),
            required_columns: Vec::new(),
            schema: None,
        });
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
        SchemaDefinition::sql_only(include_str!("../schema/report_views.sql")),
    ]
}
