# Changelog

## [0.53.0] - 2026-09-15

### Breaking

- **Breaking:** `ManagedWorkspaceReference::manifest` is a `systemprompt_models::managed::RevisionBundle` and `ManagedWorkspaceRegistration::manifest` is `&RevisionBundle`; a stored projection that does not decode as a bundle is `InvalidSpec` on read. Migrate by passing the bundle instead of `serde_json::to_value(&bundle)`.
- **Breaking:** `EvaluationRepositories::new(&DbPool, EvaluationSeams) -> Result<Self>` (and `with_admission(&DbPool, EvaluationSeams, admission)`) build every repository on the application write pool over the shared-layer seams (`AiRequestTrace`, `AiSessionProvider`, `ManagedRevisionOwnership`); `BudgetRepository`, `EvidenceRepository`, `ExecutionCapabilityRepository`, `EvaluationLifecycleRepository`, `GatewayEvaluationRepository` (`GatewaySeams`), `ExperimentRepository`, `AssignmentRepository` and `CampaignRepository` take their collaborators in `new`. Migrate by composing the seams once at the root and passing the bundle down.
- **Breaking:** `SamplingRepository`, `SampleFilter`, `SampleMode`, `SampledRequest`, `CanonicalMessage` and `EvalRepositories::sampling` are removed; `SamplerService::new(DynAiRequestTrace)` samples through `systemprompt_traits::TraceSampleFilter` and returns `TraceSample`s, `CanonicalPrompt::from_sample` builds a prompt from one, and `CanonicalPrompt`/`EvalCase` carry `ProviderId`/`ModelId` with `EvalCase::prompt` typed (`prompt_body`, `canonical_messages`, `system_prompt`, `offered_tools`, `provider`, `model` fields are gone).
- **Breaking:** `EvaluationError::BudgetExhausted { required, available }` replaces `{ spent, budget }` and is built by `EvaluationError::budget_exhausted(&ExperimentPreflight)`; `EvaluationError::Trace(AiProviderError)` and `ManagedRevisions(ManagedSkillResolverError)` replace the unused `Ai`, `RunNotFound`, `RubricNotFound`, `JudgeParse` and `ReplaySource` variants.
- **Breaking:** `CampaignRecord::status` is `models::CampaignStatus`, `ExecutionAccounting::status` and `DeterministicMeasurement::accounting_status` are `models::AccountingStatus`, `ExecutionApproval::status` is `models::ApprovalStatus`, and `ComparisonReport::variants` is `Vec<MeasurementRow>` (`MeasurementRow` / `RetainedMeasurement` are exported from `repository::experiments`). Migrate by matching the enums instead of comparing strings; the JSON wire form is unchanged.
- **Breaking:** `CampaignRecord::status` is `models::CampaignStatus`, `ExecutionAccounting::status` and `DeterministicMeasurement::accounting_status` are `models::AccountingStatus`, `ExecutionApproval::status` is `models::ApprovalStatus`, `RetainedSuggestion::status` is `models::SuggestionStatus`, and `ComparisonReport::variants` is `Vec<MeasurementRow>` (`MeasurementRow` / `RetainedMeasurement` are exported from `repository::experiments`). Migrate by matching the enums instead of comparing strings; the JSON wire form is unchanged.
- **Breaking:** `CampaignRepository::transition` takes a `CampaignTransition { expected_generation, action }` instead of an `(i64, CampaignAction)` tuple. Migrate by constructing the struct.
- **Breaking:** the unreleased migration slots are renumbered contiguously — `013_campaigns`, `014_campaign_completion`, `015_suggestion_operations` (they were 013/015/016 with 014 skipped). A database that applied the unreleased 015/016 slots must be reset or re-stamped; released databases are unaffected.

### Added

- A versioned, fail-closed evaluator capability registry covering Claude Code, OpenCode, Codex, Hermes and Claude Desktop.
- `campaigns` module: organisational optimisation campaigns — durable policy with immutable attached experiments, source-change provenance, campaign runs, holdout proposals reviewed independently of execution admission (`holdout`), retained diagnostics for blocked work (`diagnostics`), campaign reports and comparisons, and development-only suggestions with atomic reservations (`suggestions`). Migrations 013 (`eval_campaigns`, `eval_campaign_experiments`, `eval_campaign_source_changes`), 014 (`eval_campaign_diagnostics`, `eval_campaign_holdout_proposals`, `eval_holdout_content_consumption`) and 015 (suggestion `operation_key` / `operation_digest`).
- `native_proofs`: reviewed native acceptance provenance and immutable image identities; a native target stays unavailable until both the isolation and the gateway-metering proof are retained. Embedded proofs fail closed instead of panicking.
- `repository::experiments::admission` rechecks retained execution admission before every claim and spend reservation; `collections` traverses experiments by owner-scoped cursor independent of mutable timestamps; `campaign_runs`, `holdout`, `lifecycle_models` and `lifecycle_suggestion_operations` back the campaign lifecycle; `EvalCampaignId`.
- `eval_approved_operation_receipts`, `eval_execution_capabilities`, `eval_fixture_*`, `eval_session_bindings` and `eval_workers` join the declared schema.

### Changed

- `execution_accounting` returns `InvalidSpec` when a token or tool-call count is negative instead of reporting zero.
- A stored campaign, approval or suggestion status outside the declared set is `InvalidSpec` on read instead of being passed through as text.
- `capabilities`, `experiments::execution`, `repository::experiments::{evidence,lifecycle,runs}` are directory modules; `native_proofs` is `capabilities::proofs`.
- Campaign eligibility requires every frozen execution pair; terminal evidence is bound to the frozen workspace configuration; workspace inspection is bounded and unsafe materialisation paths are rejected; startup diagnostics are retained behind cleanup fences.
- Native execution and judging replay through gateway accounting: a request the gateway settles as failed spend remains authoritative over any native usage report.

### Fixed

- `CampaignRepository::create` verifies through `ManagedRevisionOwnership` that the baseline revision is held by the owner (`ResourceNotFound`) and belongs to the campaign's resource (`InvalidSpec`) before persisting the policy.
- `BudgetRepository::retain_orphaned` treats an execution `awaiting_approval` as live; its reservations stay held instead of being settled as orphans.
- The crate no longer queries `ai_requests*` or `user_sessions`: sampling, evidence audit, budget settlement and execution accounting read recorded usage through `AiRequestTrace`, and execution sessions are created and re-verified (unrevoked, unexpired, owned) through `AiSessionProvider`.
- An approved privileged operation is consumed the first time it is authorised (`status='consumed'`); a second `authorize_operation` on the same approval is a conflict instead of a silent re-authorisation.
- Execution claims order by `variant_index` and `repetition` within a creation instant, so a worker takes an experiment's baseline before its candidates instead of an arbitrary row.
- Every table the extension creates is declared by its own `SchemaDefinition` (one schema file per table), so `infra db doctor` no longer reports the evaluation tables as undeclared.

### Removed

- The never-written `eval_campaign_source_changes` table (migration 016 drops it).

## [0.52.0] - 2026-09-14

### Breaking

- The standalone judge-run surface is removed: `EvalRunRepository`, `EvalResultRepository`, `EvalRubricRepository`, `EvalJudgeCallRepository`, the `EvalRun`/`EvalResult`/`Rubric`/`JudgeVerdict` models and the `eval_runs`, `eval_results`, `eval_pairs`, `eval_judge_calls` and `eval_rubrics` tables (migration `010_drop_judge_run_tables.sql` drops them). `EvalRepositories` keeps `cases` and `sampling`. Every paid evaluation is a supervised experiment.
- Experiments carry the supervised evaluator schema: migrations `005`–`009` add managed workspace projections and assets (immutable, cleanup-only deletes), execution runtime and lifecycle columns, session bindings, approvals, cleanup records and forwarded owner constraints. `EvaluationRepositories::new(&PgPool)` bundles the experiment repositories; `EvaluationLifecycleRepository`, `GatewayEvaluationRepository`, `ExecutionCapabilityRepository` and `WorkerRepository` are new.

### Added

- Shared budget accounts have idempotent owner-scoped creation and inspection.

### Changed

- Experiment launch can reference an existing shared account. Cancelling one experiment no longer freezes the account or blocks unrelated experiments that share its cap.

### Fixed

- `reconcile_restart` retains the budget of every unsettled reservation on an execution that has finished: a request whose provider usage is recorded settles at that cost, one whose usage never arrived is charged its full reserved bound. Expired leases used to leave `reserved` held forever, exhausting the account.

## [0.49.0] - 2026-09-09

### Breaking

- **Breaking:** `AdmissionRequest` requires the resolved provider; supply it with the builder before admission.

- **Breaking:** `ExecutionLease.worker_id` and `ExperimentRepository::claim` use `EvalWorkerId`. Migrate by assigning a worker identity independently of its owning user.

### Added

- `AssignmentRepository` returns hash-verified case, rubric, bundle and configuration snapshots only for a live owner-scoped worker lease.
- `ExecutionEventRepository` records bounded, ordered progress with idempotent delivery and rejects conflicting duplicates, stale leases and revoked workers.
- Client capability validation rejects empty or unbounded versions and malformed image hashes.

- Added hashed, rotating execution capabilities bound to owner, environment, worker, session and lease fence. Admission rejects revoked workers and provider mismatches. The authenticated principal carries a fixed `user` role, not the owner's, so a run started by an administrator never authorises as one.

- `WorkerRepository` issues hashed, environment-scoped credentials with expiration and revocation.
- `EvidenceRepository` stores immutable workspaces, verifies uploaded artifact hashes and checks request references against the audit trail.
- `GatewayEvaluationRepository` binds execution sessions, reserves request budgets transactionally and settles recorded usage idempotently.

- `ExperimentRepository` and `RevisionRepository` persist immutable case, rubric and dataset inputs with owner-scoped experiment matrices.
- `BudgetRepository` distinguishes new admission from duplicate reservations, settles each request once and freezes admission after an overage.
- `experiments::scoring::score` rejects incomplete or unsupported judgments and calculates weighted outcomes using integer arithmetic.

### Changed

- Claiming limits each owner to two active executions, and lease heartbeats stop at a 30-minute execution deadline.

### Fixed

- Serialize owner-scoped claiming, cancellation and completion to prevent concurrent completions from leaving experiments running.

## [0.42.0] - 2026-08-31

### Added

- `JudgeSpec` is exported from `services`.

## [0.31.0] - 2026-08-18

### Breaking

- **Breaking:** `SampledRequest` gains a `context_id` field. Migrate struct literals by populating it from the sampled row.

### Added

- `SampleMode::Conversation` samples one transcript per `context_id` — the latest completed request, whose stored messages carry the whole conversation — so the judge grades conversations instead of isolated turns; `SampleFilter` gains `mode` and `context_id`.

### Changed

- Sampling excludes `ai_requests` rows flagged `synthetic`.

## [0.29.0] - 2026-08-05

### Added

- Initial release: evaluation tables (`eval_runs`, `eval_cases`, `eval_results`, `eval_pairs`, `eval_judge_calls`, `eval_rubrics`) installed via the extension framework.
- Production-traffic sampling from `ai_requests` with judge/replay traffic excluded from candidate selection.
- Rubric-driven LLM judge producing per-dimension scores, a 1–5 overall score, and pass/partial/fail verdicts.
- Failure replay: canonical prompt reconstruction with repair-hint injection, re-scored and linked to the original result.
- `AutoImproveLoop` orchestrating sample → judge → repair → replay → re-score with sample and budget limits.
