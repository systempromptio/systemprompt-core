# systemprompt-evaluation

Evaluation framework for the [systemprompt.io](https://systemprompt.io) platform.

Every paid evaluation is a supervised experiment: immutable case and rubric
revisions, a shared budget account with per-request reservations, leased
executions fenced by a token, evidence validated before a verdict is
recorded, and restart reconciliation that never requeues uncertain work.
Golden cases are captured from recorded traffic.

## What it provides

- **Experiments** — `eval_experiments`, `eval_executions`, revisions,
  budgets and reservations, worker leases, approvals, session bindings and
  cleanup records, installed via the extension framework.
- **Evidence** — `eval_execution_evidence` with structural validation of
  each artifact before it can support a verdict.
- **Gateway admission** — bounded spend reserved per request on a bound
  session and settled once the provider reports usage; a lease that expires
  before settlement retains its reserved bound.
- **Golden cases** — `eval_cases`, promoted from `ai_requests` with the
  sampling reader.

## Usage

The crate registers its schema through `systemprompt-extension`. The
evaluator supervisor in `systemprompt-scheduler` drives executions through
the worker routes in `systemprompt-api`; `systemprompt admin evals promote`
captures cases. See the `systemprompt` facade crate (feature `evaluation`).

## License

Business Source License 1.1 — see <https://systemprompt.io> for details.
