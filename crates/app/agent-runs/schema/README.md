# Headless Schema

PostgreSQL schema for the `systemprompt-headless` crate. Each table lives in its own `.sql` file and is embedded at compile time via `include_str!` in `extension.rs`; versioned migrations would live under `schema/migrations/` and are discovered by `build.rs`.

## Tables

| File | Table | Purpose |
|------|-------|---------|
| `headless_runs.sql` | `headless_runs` | One row per headless run: origin, owner, host, status, the `RunRequest` JSONB, the claim lease, the run's credentials and its outcome. |
| `headless_run_events.sql` | `headless_run_events` | The run's decoded `RunEvent`s in arrival order, keyed `(run_id, seq)`. |

The claim index `(status, not_before, created_at)` serves `FOR UPDATE SKIP LOCKED`; the partial lease index serves stale-run reclaim. `updated_at` is maintained by `update_timestamp_trigger()` from the database extension.

## Origins

`(origin_kind, origin_id)` names the product row that queued the run (`agent_farm_run`, `workflow_step`). Products own their rows and hold the foreign key to `headless_runs.id`; this schema references no product table.

## Lease columns

`claimed_by`, `lease_until`, `claimed_at`, `not_before` and `attempt` carry the claim. A claimed row is `preparing` or `running` with a future `lease_until`; a row whose lease has passed is reclaimed to `queued` with `attempt + 1` and `request.resume_session_id` set from `host_session_id`, or failed as `host_crashed` on its final attempt.
