# Changelog

## [0.29.0] - 2026-08-05

### Added

- Initial release: evaluation tables (`eval_runs`, `eval_cases`, `eval_results`, `eval_pairs`, `eval_judge_calls`, `eval_rubrics`) installed via the extension framework.
- Production-traffic sampling from `ai_requests` with judge/replay traffic excluded from candidate selection.
- Rubric-driven LLM judge producing per-dimension scores, a 1–5 overall score, and pass/partial/fail verdicts.
- Failure replay: canonical prompt reconstruction with repair-hint injection, re-scored and linked to the original result.
- `AutoImproveLoop` orchestrating sample → judge → repair → replay → re-score with sample and budget limits.
