# Changelog

## [0.77.0] - 2026-10-30

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
