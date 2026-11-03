# Changelog

## [Unreleased]

### Added

- `UsageRepository` (`list_counts_since`, `set_rates`) and the `brain_usage` job: Laplace-smoothed `usage_cited_rate` / `usage_ignored_rate` on `brain_facts` (migration `003_usage`, `schema::BRAIN_USAGE_COLUMNS`), read by `recall::score` and `FormulaImportance::in_context`. `set_rates` is the one write outside `FactRepository::apply`; it records no revision.
- `brain_consolidate` job (`BrainConsolidateJob::run_with`, `ConsolidationParts`, `ConsolidationInactive`): scaffolding over the `FactConsolidator` seam, inactive unless the profile enables it and a consolidator is registered.
- The session pathway in `normalise` (`EventPayload::Session`), `filter` (`CaptureSource::Session`) and routing; the producer lives in the gateway.
- Migration `002_recall.sql`: `brain_working_set` (one row per fact per reachable scope key, cascading on delete), `brain_fact_links` (the subjects a fact's own subject and `entity` fields name), `brain_facts.search_weighted` (A: subject and identity fields, B: other fields, C: statement plus synonym expansions) beside the generated `search` column, which stays for one release, and `brain_facts.observed_at`, backfilled from `created_at`. The migration backfills every index from the stored facts, kinds and topics.
- `WorkingSetRepository` (`refresh`, `rebuild_all`, `counts`, `keys_for`, `count_for`), `FactLinkRepository` (`refresh`, `rebuild_all`, `list_for`), `SearchIndexRepository` (`refresh`, `refresh_with_stored_synonyms`, `rebuild_all`, `vector_text`) and `SynonymsRepository::load`; `index::reindex_fact` refreshes all four indexes and the subject's aliases on the write connection after an insert, revision, merge or field edit.
- `services/brain/synonyms.yaml` (`SynonymsDoc`, config document `synonyms`), expanded at index time; applying it, a kind or a topic rebuilds the affected index in the apply transaction (`plane::Rebuilds`).
- `alias: true` on a kind field writes the field's value as an alias of the fact's subject; the subject id and its spaced form are always written.
- `observed_at` on entered facts, `BrainFact`, `NewFactBuilder::observed_at` and `FactFilter::with_as_of`; `as_of` on `GET /api/v1/brain/facts`, the simulator body and `brain_recall_preview`.
- The sweep rebuilds a drifted working set and counts `brain_working_set_rebuilds_total`.

### Changed

- `BrainFact` carries `usage_cited_rate` and `usage_ignored_rate`; `recall::score::ImportanceTerms` adds `usage`.

## [0.77.0] - 2026-11-01

### Added

- Initial release: the `brain_*` schema for declared kinds, topics, interests, workflow profiles and config documents, subjects with trigram-matched aliases, facts with append-only revisions, capture ledgers, recalls, outcomes and proposals.
- `BrainRepositories` over every brain table, with `FactRepository::apply` as the single path that changes a stored fact and records its revision.
- `OutboxBrainSink`, which appends brain events to the durable outbox on the producer's connection.
- `BrainExtension`, which registers the schema and migrations and enrols personal facts, recalls and interests in the user purge.
- The capture consumer and pipeline: filter, extract, validate, dedupe, route, rules, status, persist and notify stages per outbox event, under a pipeline version, with bounded retries and a daily extraction token budget.
- `BrainService`, the person-facing service behind the REST and MCP surfaces, with proposal decisions and dry-run replay of a proposal against past events (`BrainError::ProposalNotFound` for an unknown proposal).
- `BrainRecallService`, the gateway `MemoryRecall`: four retrieval tiers fused by reciprocal rank, per-fact visibility that fails closed, per-kind quotas, budget fill by in-context importance, same-turn reuse, and one `brain_recalls` row per recall carrying its project and selected workflow.
- `OutcomeService` for the `cited_in_response` and `user_corrected_fact` signals, and the `v_brain_kind_health` and `v_brain_workflow_recall` views.
- The `brain_review`, `brain_retention` and `brain_analysis` jobs; retention also bounds extractions and outcome signals by `lifecycle.results_retention_days`, and the analyses draft relevance per workflow and kind and coverage per project.
- Every `BrainService` read (`get`, `search`, `timeline`, `why`) and dispute applies recall's per-fact visibility for the caller's actor and project; a hidden fact is left out of listings and reads as `NotFound`. `BrainServiceBuilder::source_access` and `audiences` supply the checks, and a service built without a source check reads no sourced fact.
- Entering a project, group or org fact needs the authority that governs it (an admin or a workflow profile's confirm role); `BrainCaller` carries the request's actor and project.

### Changed

- The stored `filters.yaml` declaration and the interest `audience`, `granularity` and `memory`/`digest` delivery fields are accepted and have no effect in 0.77; capture filtering is the `EventFilter` seam alone.
