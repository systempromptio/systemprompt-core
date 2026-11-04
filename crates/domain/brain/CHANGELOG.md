# Changelog

## [0.78.0] - 2026-11-04

### Breaking

- **Breaking:** `recall::fusion::boost` is removed; an extension retriever's hits are a `TierHits` of `RetrievalTier::Extension` fused by `fuse` like any tier (see `recall::introduce`).
- **Breaking:** public structs gain fields, so a struct literal or exhaustive destructure no longer compiles: `NewFact.observed_at`; `FactFilter.as_of`; `DeclaredRecall.synonyms`; `DeclaredBrain.synonyms`; `ImportanceTerms.usage`; `BacklogSnapshot.{reindexed, last_round}`; `SweepReport.{working_set, working_set_rebuilt, index_backlog}`; `pipeline::EventSummary.{writes, result}`; `pipeline::PersistPlan.{observed_at, reindex}`; `TierFilter.{keys, as_of, working_set, links_weight, alias_min_sim, max_subjects}`; `recall::budget::BudgetLimits.{order, linked_share, min_fused_score, context_share, usage_weight}`.
- **Breaking:** `EventPayload` gains `Session` and `BrainError` gains `InvalidFields`. Migrate exhaustive matches by adding the arms.
- **Breaking:** `recall::budget::fill` takes 3 parameters instead of 2; `TierFilter.now` is removed in favour of `as_of`; `AliasRepository::upsert` is removed in favour of `upsert_many`; `BudgetLimits` and `TierFilter` no longer derive `Eq`; `BrainRecallStore::new` is no longer `const`.

### Added

- The recall indexes, in migration `002_recall`: `brain_working_set` (one row per fact per reachable scope key, cascading on delete), `brain_fact_links` (the subjects a fact's own subject and `entity` fields name), `brain_facts.search_weighted` (A: subject and identity fields, B: other fields, C: statement plus synonym expansions) and `brain_facts.observed_at`. `WorkingSetRepository`, `FactLinkRepository`, `SearchIndexRepository` and `SynonymsRepository` maintain them; `index::reindex_facts` refreshes them and the subject's aliases on the write connection.
- `services/brain/synonyms.yaml`, expanded at index time; applying it, a kind or a topic rebuilds the affected index in the apply transaction (`plane::Rebuilds`). `alias: true` on a kind field writes the field's value as an alias of the fact's subject.
- `observed_at` on entered facts, `NewFactBuilder::observed_at` and `FactFilter::with_as_of`. A recall with `as_of` admits only facts observed by then and valid then.
- Recall quality knobs in `retrieval.yaml`, every default preserving the measured behaviour except `fill_order`: `fill_order` (`rank`, the fused order; `importance` is the previous fill), `linked_share`, `min_fused_score` (a fact fused below it is withheld as `low_score`; a message no tier scores injects nothing), `context_share` (past it `context_cap`), `search_retry_quorum`, `search_quorum_over`, `alias_min_sim` and `max_subjects`.
- The `linked` retrieval tier: facts one `brain_fact_links` hop from a subject the message names rank on their own tier (`tiers.linked`, default `0.6`), so a neighbour never earns the subject-match importance bonus.
- Search term statistics: `brain_search_terms` (migration `004_search_terms`) holds the document frequency of every lexeme in `search_weighted`, recounted by the consumer sweep. The full-text tier plans its query from it: a fact must carry a `search_quorum` share (default `0.5`, at least one) of the message's informative words, rarest first, where a word is informative when some fact carries it and no more than `search_max_df` (default `0.2`) of the facts do.
- Extension retrievers introduce candidates. A fact a `FactRetriever` returns that no core tier found is loaded through the `recall::FactLoader` seam (`BrainRecallService::builder(..).loader(..)`; `BrainRecallStore` implements it), admitted by `recall::admit::admitted_by_sql` and only then fused as the `extension` tier. A rejected fact is withheld as `stale`, `review_due` or `disputed` where the lifecycle path names the reason and as `scope` otherwise. A failing loader is logged and introduces nothing. `extension_max_candidates` (default `24`) caps the introduced ids; with `tiers.extension: 0.0` nothing is loaded.
- The citation loop feeds the fill. `usage_weight` (default `0.0`) adds `usage_weight × (cited_rate − ignored_rate)` to each candidate's fused score before `min_fused_score` and the fill order; `usage_min_recalls` (default `0`) keeps a fact at the prior until the `brain_usage` job has seen that many injections. With `min_fused_score` above zero, a net-ignored fact that is withheld is never injected again and so never re-rated.
- The daily `brain_usage` job (02:45) and `UsageRepository`: Laplace-smoothed `usage_cited_rate` and `usage_ignored_rate` on `brain_facts` (migration `003_usage`), from the recalls and `cited_in_response` outcomes of the last `lifecycle.results_retention_days`. The write records no revision, the one exception to `FactRepository::apply` as the sole mutator. Metric `brain_usage_updated_total`.
- The daily `brain_consolidate` job (03:45; `BrainConsolidateJob::run_with`, `ConsolidationParts`, `ConsolidationInactive`). It runs only when `brain.consolidation.enabled` is on and a `FactConsolidator` is registered; otherwise it succeeds with zero items and logs the reason once. Candidates are entered as `proposed` facts of the kind they name. Metric `brain_consolidations_total{outcome}`.
- The session pathway in `normalise` (`EventPayload::Session`), `filter` (`CaptureSource::Session`) and routing. With no filter gathering session events, each one is recorded as `no_filter` and nothing is extracted.
- `BacklogSnapshot::last_round` and the backlog read behind `checks.brain` on `GET /health/detail`.
- Metrics `brain_round_events`, `brain_round_facts_reindexed`, `brain_index_statements_total`, `brain_round_second_passes_total` and `brain_working_set_rebuilds_total`.

### Changed

- Recall tiers read the precomputed working set. A recall builds the principal's held keys once (`org`, `user:<id>`, `project:<attribution>` and every audience subject a resolver enumerates through `NotificationAudienceResolver::held`), and each tier runs over the `brain_working_set` rows under those keys joined to `brain_facts`. `brain.recall.working_set: false` runs the scope-predicate tier SQL.
- The subject tier follows one `brain_fact_links` hop: a candidate whose own subject is named scores `1.0`, and one linked to a named subject scores `links_weight` (default `0.6`). The full-text tier ranks `search_weighted` with `ts_rank_cd`, normalised by length.
- Recall is deterministic. Every tier orders by rank, then stored importance, then fact id, and `now` is read once per recall and is the only clock the tiers, the lifecycle check and the importance score use. A fact's recency age counts in whole days.
- Withheld reasons report what happened. `stale`, `review_due` and `disputed` are counted from the candidate set in one grouped query; the fact-count cut is `max_facts`, and `budget` means only the token cut. `v_brain_workflow_recall` gains `withheld_max_facts`, `withheld_disputed` and `late_blocks`, and its injected and withheld sums count injecting rows only.
- The in-context importance score reads usage. The `importance.yaml` defaults for `in_context.usage` change from `0`/`0` to `cited_rate_weight: 0.2`, `ignored_penalty: 0.1`; a tree that declared the weights explicitly is unaffected. The why-view adds a `usage` term.
- A drain round commits once. It claims up to `consumer.batch` events on one transaction, runs each in its own savepoint and acknowledges each finished event on that transaction. A transient failure holds the event back for `poll_secs` and the round goes on, ending early after more than three transient failures. A lost round is claimed again whole.
- A consumer round indexes as a set. The reindex runs once per round over every fact the round landed on (`index::reindex_facts`, 8 statements), and the round preloads its result-ledger claims, alias resolutions, dedupe rows and tombstones in four set statements (`pipeline::RoundCache`). A healthy entered fact costs about 4.9 statements at `consumer.batch` 32. The projection drain shares the path.
- The recall indexes an upgrade needs are queued rather than backfilled at migration. Migration `002_recall` is DDL plus one keys-only insert into `projection_backlog` for the `brain_fact_index` projection; the consumer sweep drains it (256 keys a batch, ten seconds a sweep) through the write path's code, flooring `observed_at` to `created_at`, then rebuilds a drifted working set. `SweepReport::index_backlog` reports the drain, and until it finishes an upgraded fact is pending for recall.
- `recall.deadline_ms` defaults to `200`. A recall the gateway runs past the deadline stores its block, and the turn's next request reuses it with the descriptor suffix `reused_after_deadline`.
- The write path reads the `synonyms` document at most once per 30 s per process (`SynonymsRepository::load_cached`); a plane apply that changes it invalidates the cache before rebuilding the search vectors.
- `brain_facts{status}` and `brain_disputes_open` refresh after a round that wrote, staled or disputed facts, at most every five seconds, as well as on the sweep.
- `capture.sources` defaults to `[document, comment, entered]`.
- An interest's `owner` may be a user id or an email. The plane apply resolves it and refuses the apply with a validation error when no user matches; the stored declaration keeps the value as written, so a tree shared across instances hashes the same.

### Fixed

- A subject no index admits fails alone. `SubjectKey` bounds the kind to 1..=64 and the id to 1..=255 characters where a subject enters: `BrainService::enter` (`invalid_subject`), the candidate stage (rejected by `core.subject`) and the consolidation job. A constraint violation fails the event permanently instead of being retried.
- The round runs under a savepoint taken before its first event. When the set reindex fails, that pass is rolled back whole and the events run again with the indexes written per event, so the refused event fails inside its own savepoint and is recorded as any per-event failure, while the others keep their facts. The model is not asked twice: the round remembers each first-pass extraction and re-records its spend rows.
- `refresh_projection` runs the `brain_fact_index` batch under a savepoint; when it fails the keys are reindexed one by one, a key failing on `RepositoryError::Constraint` is parked past the retry budget, and the rest of the batch commits. Any other failure re-queues the batch.
- Ownership of a shared alias is a rule: the fact with the greatest `(updated_at, id)` among those that contribute it. `brain_aliases` records `owner_fact_id` and `owner_updated_at` (migration `005_alias_owner`), so a full reindex in any batch order lands every alias where the consumer rounds left it. Facts are stamped with `clock_timestamp()` on insert and revision. `AliasRows::collect` takes `(AliasOwner, Vec<String>)`.
- An erased, retired, staled or superseded fact holds no alias. `index::release_alias_owners` deletes the rows the fact owns and reindexes every live fact that could still contribute them (`AliasRepository::list_contributors`), from `FactRepository::apply`, `FactRepository::erase`, the retention purge and `reindex_facts`.
- `NewFailure::scrub_reason` replaces each NUL byte in a failure reason before the write, which `TEXT` refuses.
- A failed statement recording an attempt or closing a result row on the round's transaction ends the round, which rolls back and retries, rather than being swallowed before a commit that would roll back silently.
- A tombstone written while a consumer round runs is honoured: a cache-covered snapshot is re-checked against `brain_tombstones` before the candidate is written.
- `BrainService::enter` refuses an erased identity at entry with nothing queued, computing the identity with the pipeline's function after alias resolution. The consumer's rejection stays as the backstop.
- `BrainService` field edits merge onto the stored fields: a key replaces its field, a key sent as `null` removes an optional field, and keys left out are kept. The merged fields are checked against the kind (`BrainError::InvalidFields`) before the statement is re-rendered and the identity compared.
- `consumer.source_timeout_ms` bounds only the consumer's source reads; recall's source checks use `brain.recall.source_timeout_ms`.

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
