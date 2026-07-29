# Outstanding work in core

Companion to `bug.md`. Ordered by severity. Line numbers verified against
HEAD `42cb49b34`.

---

## P0 — data loss, ship first

### 1. Remove `delete_orphaned_mcp_executions`

`crates/infra/database/src/repository/cleanup.rs:37-48` and its caller at
`crates/app/scheduler/src/jobs/database_cleanup.rs:52`.

Preferred: delete the function outright. Tool executions should outlive their
context — every consumer aggregates by `user_id`, and `context_id` carries no
foreign key, so its absence was never an integrity violation.

If orphan collection must survive, it needs a mandatory age parameter and a
`warn!` with the count. It must never be able to delete a same-day row.

- [ ] Remove function + caller, or gate on age
- [ ] Update `crates/tests/unit/infra/database/src/repository/cleanup.rs`
- [ ] CHANGELOG under `### Fixed`, consumer-facing

### 2. Stop `cleanup_empty_contexts` collecting audit-bearing contexts

`crates/app/scheduler/src/repository/analytics/mod.rs:24-50`.

Add `NOT EXISTS` guards for `mcp_tool_executions` and `governance_decisions`
beside the existing `task_messages` join. A context holding audit rows is not
empty regardless of age.

- [ ] Add the guards
- [ ] Raise the default window from 1 hour (`jobs/cleanup_empty_contexts.rs:42`)
      to something defensible once (3) makes it configurable

### 3. Drop `database_cleanup` from the default bootstrap set

`crates/shared/models/src/services/scheduler.rs:90-95`. An irreversible deleter
should not run on every process start. Nightly is enough.

- [ ] Remove from `default_bootstrap_jobs()`
- [ ] **Breaking-ish**: note in CHANGELOG that deployments relying on the
      boot-time sweep must now list it explicitly

---

## P1 — the architectural gap that made P0 unfixable downstream

### 4. Make job parameters reach scheduled runs

The plumbing exists and is one line short of working.

- [ ] Add `#[serde(default)] pub parameters: HashMap<String, String>` to
      `JobConfig` (`crates/shared/models/src/services/scheduler.rs:9-25`)
- [ ] Thread through `JobDispatch` (`crates/app/scheduler/src/services/scheduling/dispatch.rs:30,42`)
      exactly as `enforce` is threaded
- [ ] At `dispatch.rs:175`, chain `.with_parameters(...)` onto the existing
      `.with_enforce(enforce)` — today scheduled runs always get an empty map
      while `services/job_execution.rs:199` (manual/API) gets a populated one
- [ ] Add a builder (`JobConfig::with_parameters`) for parity with
      `with_owner` / `with_schedule` / `with_enforce`

### 5. Convert hardcoded retention windows to parameters

Each keeps its current value as the default, so behaviour is unchanged unless a
deployment overrides it:

- [ ] `cleanup_empty_contexts` — 1 hour (`jobs/cleanup_empty_contexts.rs:42`)
- [ ] `delete_old_logs` — 30 days (`jobs/database_cleanup.rs:58`)
- [ ] `cleanup_inactive_sessions` — 1 day (`jobs/cleanup_inactive_sessions.rs:43`)
- [ ] `mcp_session_cleanup` — 7 days (`crates/domain/mcp/src/jobs/mcp_session_cleanup.rs:14`)
- [ ] `cleanup_anonymous_users` — 30 days (`crates/domain/users/src/jobs/cleanup_anonymous_users.rs:15`)
- [ ] Document the parameter names in the `JobConfig` rustdoc and each job's
      `description()`

### 6. Make every deleting job honour `enforce`

`enforce` is documented at `scheduler.rs:21-24` as the opt-in for destructive
actions, but only `behavioral_analysis` and `malicious_ip_blacklist` read it.

- [ ] `database_cleanup` and `cleanup_empty_contexts` consult `ctx.enforce()`
- [ ] When false, report would-delete counts instead of deleting — the pattern
      already used at `jobs/behavioral_analysis.rs:219-225`
- [ ] Audit the remaining jobs for the same omission

---

## P2 — operability

### 7. Fix `admin session login` for cloud profiles

Currently impossible to query a cloud deployment from the CLI: `switch` works,
`login` dies canonicalizing container paths locally
(`crates/shared/models/src/paths/system.rs:26,40-41`,
`crates/shared/models/src/paths/build.rs:84`).

- [ ] Skip local canonicalization when the profile targets remote execution, or
      resolve container paths lazily at use
- [ ] Regression test: `session switch <cloud>` then `session login` succeeds
      with no local `/app`
- [ ] This blocks incident response — an operator cannot read production data
      while the site is misbehaving

### 8. Stop GeoIP disabling itself silently

- [ ] `crates/infra/cloud/src/profile_authoring/cloud_builder.rs:112` — preserve
      an existing `geoip_database` instead of authoring `None` over it
- [ ] `crates/app/runtime/src/context/context_loaders.rs:22-54` — hard-error when
      an **explicitly configured** mmdb is unreadable; keep the warning only for
      "not configured". `crates/infra/config/src/path_validation.rs:36-39`
      currently makes both merely optional
- [ ] Distinguish the two failure modes in diagnostics: no reader vs. client IP
      resolving to a private address (`extractor/geoip.rs:24-32` +
      `middleware/client_addr.rs:42-78`). Today both present as NULL country

### 9. Offer a `country` backfill

`crates/domain/analytics/src/repository/session/mutations.rs:178,198` writes
`country` on INSERT only, with no UPDATE path anywhere. Enabling GeoIP therefore
appears to do nothing for every existing row.

- [ ] Backfill job or CLI command for rows with non-private `ip_address` and
      NULL `country`

---

## P3 — correctness of the identity model

### 10. There is no canonical identity

Surfaced while investigating whether a user's tool calls were deleted or merely
detached. Worth a design decision rather than a patch:

- `users` has no stable key across auth paths — `create` mints a fresh UUID
  (`crates/domain/users/src/repository/user/operations.rs:22-59`), federated
  sign-in keys only on `(issuer, external_sub)`
  (`crates/domain/users/src/repository/federated_identity.rs:29-98`)
- `email` is the de-facto key but is `VARCHAR` and never normalised — `Ed@x.com`
  and `ed@x.com` both insert (`crates/domain/users/schema/users.sql:1-15`)
- Unverified federated identities get a synthetic
  `{hash}@{issuer}.federated.local` that by construction never matches the real
  account (`federated_identity.rs:112-147`) — deliberate, but it means one human
  reliably becomes several rows
- **No anonymous→registered promotion and no merge routine exists.** A visitor
  who uses the product before signing up has that activity stranded on an
  anonymous id that dashboards filter out, so their history looks deleted

- [ ] Decide: canonical `person_id`, or normalised email as the key, or an
      explicit account-link step at signup
- [ ] Until then, document that per-user aggregates undercount

---

## Verification

Once P0/P1 land, the demo should re-enable the core jobs and retire its local
`retention` workaround (`systemprompt-demo/extensions/web/jobs/src/retention.rs`,
`services/scheduler/config.yaml`).

Regression worth adding to core's own suite: write an `mcp_tool_executions` row
whose `context_id` is absent from `user_contexts`, run the full cleanup job set,
and assert the row survives.
