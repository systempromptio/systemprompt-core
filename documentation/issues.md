# Issue Reports

How users and their agents report bugs and give feedback, and how an operator triages those reports.

An issue report is a durable record owned by the person who filed it. It has a kind (`bug` or `feedback`), a severity, a status, and optional file attachments. When an agent files a report on a user's behalf, the report records that agent. Reports live in the files domain (`crates/domain/files/src/issues`), and their attachments are stored as document-store blobs.

## Enabling

Issue reports are off by default. Turn them on in `services/config/files.yaml`:

```yaml
files:
  issues:
    enabled: true
    maxReportsPerHour: 20   # new reports per reporter per hour; repeats are not counted
    maxAttachments: 5       # files per report; 0 disables attachments
    retentionDays: 90       # how long a resolved or closed report is kept
    triageRoles: [admin]    # roles that see and triage every report
```

Attachments are document-store uploads, so they require `files.documents.enabled: true`. If issue reports are on and the document store is off, reports still work without attachments, and startup validation warns about it.

To announce new reports to the people who triage them, declare the `issues` notification stream with that audience. See [Notifications](notifications.md).

```yaml
# services/notifications/streams.yaml
streams:
  - id: issues
    name: Issue reports
    audience: [role:admin]
    attention:
      default: agent
```

## What a report records

- **What happened, in the reporter's words.** A title of up to 200 characters, a body of up to 20,000 characters, and an optional `client` object (product, version, OS) of up to 8 KiB.
- **Where it happened, as identifiers.** The platform captures the filing request's `trace_id`, `context_id`, `session_id`, `task_id` and, for an MCP call, the `mcp_execution_id`. The agent does not supply these. Triage follows these identifiers to the platform's own records, for example `systemprompt infra logs trace show <trace_id>`.
- **No copied logs.** A report holds identifiers, not log lines. Credentials that might appear in a log therefore never enter the report, and the underlying records expire with normal log retention.

## Repeats fold into one report

Each report has a fingerprint: a SHA-256 of its kind, the filing agent, and either a caller-supplied key or the normalised title. While a report is `open`, `triaged` or `needs_info`, filing another report with the same fingerprint increments its `occurrences` and updates `last_seen_at` instead of creating a new row. If the repeat has a higher severity, the report's severity is raised. A repeat announces nothing.

The fold is a single `INSERT … ON CONFLICT` against a partial unique index on `(reporter, fingerprint)` over active reports. An agent that retries a failing call a thousand times produces one report with `occurrences: 1000` and one notification.

When a report is resolved or closed, the next report with the same fingerprint opens a new report.

## Limits

- **Rate.** Each reporter may file at most `maxReportsPerHour` new reports. The check runs in the same transaction as the insert, so concurrent filings cannot exceed it. A repeat that folds into an existing report does not count.
- **Attachments.** A report holds at most `maxAttachments` files. Attaching locks the report row, so concurrent attaches cannot exceed the cap.
- **Size.** Attachments use the resumable upload limits and per-user quota in `files.documents.uploads`.

## Attachments

An attachment, such as a zip of diagnostics or a screenshot, takes three steps:

1. **Declare** the file (name, MIME type, size, SHA-256). The answer is a set of signed upload links that are valid for 15 minutes and carry no other permission.
2. **Upload** the bytes to those links, in parts, with retries. The bytes never pass through the MCP tool or the report API. When the upload completes, its hash is verified and the upload content checks run. Any archive is checked by the zip-bomb guard, which reads the central directory and refuses one whose entry count or decompressed size exceeds its limits.
3. **Attach** the completed upload. Only a completed upload owned by the reporter can be attached, and only to that reporter's own active report.

Archives are stored as opaque blobs and are never extracted on the server. The blob collector keeps any blob that a report references, and the blob becomes collectable once the report is purged.

## Lifecycle

| Status | Meaning |
|--------|---------|
| `open` | Filed, not yet looked at |
| `triaged` | Accepted and being worked on |
| `needs_info` | Waiting on the reporter |
| `resolved` | Fixed or answered |
| `closed` | Closed without a fix |

Whenever triage changes a report's status, an `issue.status_changed` notification goes to the reporter, with attention `agent`, so the reporter's agent sees it in its inbox. Resolved and closed reports are deleted `retentionDays` after they close by the cluster job `issue_report_retention`, together with their attachment rows. When a user is deleted, their reports are deleted with them.

## Who can see a report

A report is visible to the person who filed it and to any active user holding one of the `triageRoles`. It is visible to no one else: anyone else asking for it gets `404`, the same answer as for a report that does not exist. Notification visibility follows the same rule, so the `issues` stream never shows a report's title to a member of the stream audience who is not allowed to see the report.

## Surfaces

### MCP tools

Core provides the tools as a library, `systemprompt_runtime::issues::mcp::IssuesMcpTools`, which an install mounts on an MCP server it runs. An install typically runs a dedicated issue-report server with `oauth.scopes: [user]`, so every signed-in user's agent can file reports.

| Tool | Purpose |
|------|---------|
| `report_issue` | File a report. The call's trace, context, session, task and MCP execution are recorded automatically. |
| `issue_attachment` | `declare` an upload and receive signed links, then `attach` the completed upload. |
| `issues_list` | The caller's own reports, most recently seen first. |
| `issue_status` | One report, with its status, triage note, occurrence count and attachments. |

A refusal is a tool result with `isError: true` whose `structuredContent.code` is one of `forbidden`, `disabled`, `invalid_params`, `rate_limited`, `not_found`, `not_active`, `attachments_full`, `upload_not_ready` or `uploads_unavailable`.

### HTTP API

| Method and path | Purpose |
|-----------------|---------|
| `POST /api/v1/issues` | File a report |
| `GET /api/v1/issues` | The caller's own reports |
| `GET /api/v1/issues/{id}` | One report, for its reporter or a triager |
| `POST /api/v1/issues/{id}/attachments` | Declare an attachment upload |
| `POST /api/v1/issues/{id}/attachments/{upload_id}` | Attach a completed upload |
| `GET /api/v1/issues/admin` | All reports, for triagers |
| `PATCH /api/v1/issues/admin/{id}` | Change a report's status and note |

### CLI

```bash
systemprompt admin issues list --status open
systemprompt admin issues show <id>
systemprompt admin issues triage <id> --status triaged --note "Reproduced"
```

`show` prints the report's context identifiers together with the commands that inspect them.
