# Changelog

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
