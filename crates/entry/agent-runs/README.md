# systemprompt-agent-runs-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-agent-runs-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-agent-runs-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-agent-runs-entry?style=flat-square)](https://docs.rs/systemprompt-agent-runs-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The agent-run HTTP routes of systemprompt.io: queue, read, cancel and stream a caller's durable headless runs.

**Layer**: Entry. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Routes (`http` feature)

The `agent_runs_api` extension declares `/api/v1/agent-runs`. The API server mounts the declaration without naming this crate, on every node role that serves the agent protocol (`all`, `api`, `worker`):

| Method | Path | |
|---|---|---|
| `POST` | `/` | Queue a run under a registered origin with an idempotency key |
| `GET` | `/{id}` | Read a run |
| `POST` | `/{id}/cancel` | Cancel a queued run, or record the request for a running one |
| `GET` | `/{id}/events` | Page through the run's events |
| `GET` | `/{id}/events/stream` | The events as SSE, resumable with `Last-Event-ID` |
| `GET` | `/{id}/outputs` | List the run's output files |
| `GET` | `/{id}/outputs/{*path}` | Read one output file |

Every route requires a user or admin session (user context middleware) and is rate limited per caller under the `agent_runs` scope on the tasks budget. A run another user queued answers 404. The server owns the run's filesystem: the request's `workspace` is replaced with the run layout's, and a request that names server paths or a stdio MCP server is refused.

The run repository comes from the `agent_runs` extension (`systemprompt-agent-runs`), which also provides the A2A run launcher spawned agents queue task-backed runs through. A profile that disables `agent_runs` or `agent_runs_api` mounts no agent-runs route.
