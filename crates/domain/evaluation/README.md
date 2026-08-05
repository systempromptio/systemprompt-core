# systemprompt-evaluation

Evaluation framework for the [systemprompt.io](https://systemprompt.io) platform.

Every AI request the platform serves is already recorded — prompts, offered
tool definitions, models, wire payloads, tool calls, cost, and latency. This
crate closes the loop on that trace: it samples production requests, scores
them against configurable rubrics with an LLM judge, and replays failures with
a repair hint so the repaired trajectory is scored and recorded alongside the
original.

## What it provides

- **Evaluation tables** — `eval_runs`, `eval_cases`, `eval_results`,
  `eval_pairs`, `eval_judge_calls`, `eval_rubrics`, installed via the
  extension framework.
- **Sampling** — candidate selection from `ai_requests`, excluding the
  framework's own judge and replay traffic.
- **Judge** — rubric-driven structured scoring (1–5 overall, per-dimension
  scores, pass/partial/fail verdicts) through any configured AI provider.
- **Replay** — canonical prompt reconstruction plus repair-hint injection,
  re-scored by the same judge and linked to the failing result.
- **Auto-improve loop** — sample → judge → repair → replay → re-score,
  designed to run as a scheduled job or on demand from the CLI.

## Usage

The crate registers its schema through `systemprompt-extension`; services are
constructed with a database pool and an `AiService` from `systemprompt-ai`.
See the `systemprompt` facade crate (feature `evaluation`) and the
`systemprompt admin evals` CLI command group.

## License

Business Source License 1.1 — see <https://systemprompt.io> for details.
