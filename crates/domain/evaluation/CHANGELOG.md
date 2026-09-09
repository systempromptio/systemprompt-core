# Changelog

## [Unreleased]

### Breaking

- **Breaking:** `ExecutionLease.worker_id` and `ExperimentRepository::claim` use `EvalWorkerId`. Migrate by assigning a worker identity independently of its owning user.

### Added

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
