//! The brain tables: each table's declarative DDL and the columns the
//! schema validator requires of it, and the two report views over them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub type SchemaEntry = (&'static str, &'static str, &'static [&'static str]);

pub const BRAIN_SCHEMAS: &[SchemaEntry] = &[
    (
        "brain_fact_kinds",
        include_str!("../schema/brain_fact_kinds.sql"),
        &["id", "body", "source"],
    ),
    (
        "brain_topics",
        include_str!("../schema/brain_topics.sql"),
        &["id", "body", "source"],
    ),
    (
        "brain_interests",
        include_str!("../schema/brain_interests.sql"),
        &["id", "owner_user_id", "body", "source"],
    ),
    (
        "brain_workflows",
        include_str!("../schema/brain_workflows.sql"),
        &["id", "body", "source"],
    ),
    (
        "brain_config_docs",
        include_str!("../schema/brain_config_docs.sql"),
        &["id", "body", "source"],
    ),
    (
        "brain_pipeline_versions",
        include_str!("../schema/brain_pipeline_versions.sql"),
        &[
            "version",
            "declared_hash",
            "code_version",
            "prompt_versions",
        ],
    ),
    (
        "brain_subjects",
        include_str!("../schema/brain_subjects.sql"),
        &["subject_kind", "subject_id"],
    ),
    (
        "brain_aliases",
        include_str!("../schema/brain_aliases.sql"),
        &["id", "subject_kind", "subject_id", "alias_norm"],
    ),
    (
        "brain_facts",
        include_str!("../schema/brain_facts.sql"),
        &[
            "id",
            "kind",
            "subject_kind",
            "subject_id",
            "scope_level",
            "statement",
            "source",
            "source_kind",
            "status",
            "identity_hash",
            "statement_hash",
            "revision",
            "owner_user_id",
            "search",
            "search_weighted",
            "observed_at",
        ],
    ),
    (
        "brain_fact_revisions",
        include_str!("../schema/brain_fact_revisions.sql"),
        &["id", "fact_id", "revision", "change", "status", "statement"],
    ),
    // -- recall redesign (write side) --------------------------------------
    (
        "brain_working_set",
        include_str!("../schema/brain_working_set.sql"),
        &["scope_key", "fact_id", "importance_base"],
    ),
    (
        "brain_fact_links",
        include_str!("../schema/brain_fact_links.sql"),
        &["fact_id", "subject_kind", "subject_id", "via_field"],
    ),
    // -- end recall redesign (write side) ----------------------------------
    (
        "brain_extractions",
        include_str!("../schema/brain_extractions.sql"),
        &["id", "outbox_id", "pipeline_version", "extractor", "kind"],
    ),
    (
        "brain_event_results",
        include_str!("../schema/brain_event_results.sql"),
        &["outbox_id", "pipeline_version", "outcome"],
    ),
    (
        "brain_failures",
        include_str!("../schema/brain_failures.sql"),
        &["id", "outbox_id", "stage", "reason"],
    ),
    (
        "brain_disputes",
        include_str!("../schema/brain_disputes.sql"),
        &["id", "fact_id", "status"],
    ),
    (
        "brain_tombstones",
        include_str!("../schema/brain_tombstones.sql"),
        &["kind", "identity_hash"],
    ),
    (
        "brain_recalls",
        include_str!("../schema/brain_recalls.sql"),
        &[
            "id",
            "principal_user_id",
            "gateway_conversation_id",
            "rendered_block",
            "as_of",
        ],
    ),
    (
        "brain_outcomes",
        include_str!("../schema/brain_outcomes.sql"),
        &["id", "recall_id", "fact_id", "signal"],
    ),
    (
        "brain_proposals",
        include_str!("../schema/brain_proposals.sql"),
        &["id", "analysis", "kind", "status"],
    ),
    (
        "brain_search_terms",
        include_str!("../schema/brain_search_terms.sql"),
        &["word", "ndoc"],
    ),
];

pub const BRAIN_VIEWS: &[&str] = &[
    include_str!("../schema/v_brain_kind_health.sql"),
    include_str!("../schema/v_brain_workflow_recall.sql"),
];

// -- usage (R6) -----------------------------------------------------------
/// The `brain_facts` columns migration `003_usage` adds and the
/// `brain_usage` job writes; recall reads the two rates.
pub const BRAIN_USAGE_COLUMNS: &[&str] =
    &["usage_cited_rate", "usage_ignored_rate", "usage_updated_at"];
// -- end usage (R6) -------------------------------------------------------
