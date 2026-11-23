//! What `AiExtension` contributes: its tables and triggers, the AI jobs, the
//! retention purges and orphan sweep, the two rollup drains bound to the
//! kernel's `backfill_rollups` job, and the request-attribution lookup it
//! provides. Ordering against `users` and `mcp` is the reference
//! composition's, asserted by the facade's `composition_order` test.

use std::any::TypeId;

use systemprompt_ai::AiExtension;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::projection::kernel::BackfillRollups;
use systemprompt_extension::{Contributions, Extension, RetentionDecl};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::attribution::DynRequestAttributionLookup;

fn contributed() -> ContributionSink {
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("ai"), &mut sink);
    AiExtension
        .contribute(&mut c)
        .expect("the ai member contributes");
    sink
}

#[test]
fn metadata_names_the_ai_member() {
    let meta = AiExtension.metadata();
    assert_eq!(meta.id, "ai");
    assert_eq!(meta.name, "AI");
    assert!(!meta.version.is_empty());
}

#[test]
fn the_ai_member_declares_its_tables_in_order() {
    let sink = contributed();
    let names: Vec<&str> = sink
        .schemas()
        .iter()
        .filter_map(|s| s.table.as_deref())
        .collect();
    assert_eq!(
        names,
        vec![
            "ai_requests",
            "ai_request_attributions",
            "ai_request_client_evidence",
            "ai_request_messages",
            "ai_request_tool_calls",
            "ai_tool_catalogs",
            "ai_request_payloads",
            "ai_safety_findings",
            "ai_quota_buckets",
            "ai_gateway_policies",
            "ai_gateway_thought_signatures",
            "ai_usage_rollup_hourly",
            "ai_context_rollup_hourly",
            "user_provider_credentials",
            "ai_catalog_legacy_map",
        ]
    );
}

#[test]
fn the_ai_member_contributes_its_jobs() {
    let sink = contributed();
    let mut names: Vec<&str> = sink.jobs().iter().map(|job| job.job().name()).collect();
    names.sort_unstable();
    assert_eq!(names.len(), 3, "{names:?}");
}

#[test]
fn the_ai_member_contributes_three_purges_and_one_sweep() {
    let sink = contributed();
    let purges = sink
        .retention()
        .iter()
        .filter(|decl| matches!(decl, RetentionDecl::Purge(_)))
        .count();
    let sweeps = sink
        .retention()
        .iter()
        .filter(|decl| matches!(decl, RetentionDecl::Sweep(_)))
        .count();
    assert_eq!((purges, sweeps), (3, 1));
}

#[test]
fn the_ai_rollups_are_bound_to_the_kernel_backfill_drainer() {
    let sink = contributed();
    let projections: Vec<&str> = sink
        .projections()
        .iter()
        .filter(|entry| entry.is_for::<BackfillRollups>())
        .map(|entry| entry.drain().projection)
        .collect();
    assert_eq!(
        projections,
        ["ai_usage_rollup_hourly", "ai_context_rollup_hourly"]
    );
    assert_eq!(sink.projections().len(), 2);
}

#[test]
fn the_ai_member_provides_its_lookups_its_sealed_stores_and_the_inference_provider() {
    let sink = contributed();
    let provided: Vec<(TypeId, bool)> = sink
        .provides()
        .iter()
        .map(|p| (p.type_id(), p.is_sealed()))
        .collect();
    assert_eq!(
        provided,
        [
            (TypeId::of::<DynRequestAttributionLookup>(), false),
            (
                TypeId::of::<systemprompt_traits::DynToolCallIntentClaims>(),
                true
            ),
            (
                TypeId::of::<std::sync::Arc<systemprompt_ai::repository::AiRepositories>>(),
                true
            ),
            (
                TypeId::of::<systemprompt_traits::DynSafetyFindingsReader>(),
                true
            ),
            (
                TypeId::of::<systemprompt_traits::DynCatalogMigrationPort>(),
                true
            ),
            (TypeId::of::<systemprompt_models::ai::DynAiProvider>(), true),
        ]
    );
}

#[test]
fn no_reporting_capture_or_privacy_sql_is_contributed() {
    assert!(
        contributed()
            .schemas()
            .iter()
            .all(|schema| !schema.sql.contains("reporting")),
        "the reporting projection is retired"
    );
}

#[test]
fn message_count_is_maintained_by_a_statement_trigger() {
    let sink = contributed();
    let counter = sink
        .schemas()
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("EXECUTE FUNCTION sp_ai_request_message_count")
        })
        .count();
    assert_eq!(counter, 1, "message_count trigger contributed once");
}
