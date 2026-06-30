# Ticket: CLI session-switching friction (profile context + silent wrong-target jobs)

**Filed:** 2026-06-30
**Component:** `systemprompt` CLI — `admin session`, profile/session resolution, `infra jobs run`
**Severity:** Medium (developer-experience + correctness footgun)
**Reported from:** local web-dev workflow (systemprompt-web), bundling/publishing assets

## Summary

Switching the active session/profile is unreliable when the currently-active
profile's session token has expired. `systemprompt admin session switch <name>`
refuses to run, returns a misleading error, **and still exits 0**. Meanwhile
job recipes (`just publish`, `just web-build`, etc.) silently execute against
whatever profile happens to be active — which, after a token expiry, may be a
**remote/prod** profile when the developer intends **local**. The only reliable
workaround is appending `--profile local` to every individual command.

## Environment / preconditions

- Multiple profiles configured: `local`, `systemprompt-prod` (active), `dryrun`.
- All sessions show `session_status: expired` (`admin session list`).
- Active profile is `systemprompt-prod` (a `remote` routing target).

## Observed behaviour

### 1. `session switch` requires context of the profile you are switching *away* from

```
$ systemprompt admin session switch local
[profile: systemprompt-prod (cloud) | tenant: 999bc...]
Error: This command requires full profile context
$ echo $?
0
```

Problems:
- It loads/validates the **currently active** (`systemprompt-prod`) context
  before performing the switch. If that session is expired, the switch fails —
  even though the whole point is to leave that profile for a different one.
  Chicken-and-egg: you cannot switch away from a broken session.
- The error message ("requires full profile context") does not say *which*
  profile is the problem, *why*, or *what to do* (re-auth? use `--profile`?).
- **Exit code is 0 on failure.** Any script/recipe branching on `$?` treats the
  failed switch as success and proceeds against the wrong target.

### 2. Job recipes silently target the active (possibly remote/prod) profile

```
$ just publish
target/release/systemprompt infra jobs run publish_pipeline
[profile: systemprompt-prod (cloud) | tenant: 999bc...]
Error: This command requires full profile context
error: Recipe `publish` failed on line 313 with exit code 1
```

Here it errored, but the failure mode is the concern: a local-dev recipe ran
against a **remote prod profile** purely because that profile was active. With a
valid prod token this would have executed a write/deploy job against production
instead of local — a silent wrong-target action. (This footgun is already
documented as tribal knowledge: "jobs on the wrong profile deploy silently to
the wrong target.")

### 3. Per-command `--profile` override works under the exact same state

```
$ systemprompt --profile local infra jobs run bundle_css        # OK
$ systemprompt --profile local infra jobs run copy_extension_assets  # OK
$ systemprompt --profile local infra jobs run publish_pipeline  # OK
```

The same expired-session state that blocks `session switch local` does **not**
block `--profile local`. So the override path resolves local context fine; only
the persistent-switch path demands full (live) context of the outgoing profile.
This inconsistency is the core bug.

## Impact

- Developers must remember to append `--profile local` to **every** command;
  forgetting it runs against prod.
- `just` recipes that omit an explicit `--profile` inherit the active profile,
  making the safe path the non-default one.
- A wrong but plausible mental model ("I switched to local") combined with a
  silent/zero-exit failure can route destructive job runs at production.

## Suggested fixes (in priority order)

1. **`session switch` must not require live context of the outgoing profile.**
   Switching the active pointer is metadata-only; validate the *target* profile
   exists, then switch. Re-auth of the target can happen lazily on next use.
2. **Return a non-zero exit code on any `session switch` failure.** Recipes and
   scripts rely on `$?`.
3. **Actionable error text.** Name the offending profile and the remedy, e.g.
   "Active profile 'systemprompt-prod' session expired. Run `systemprompt cloud
   login --profile systemprompt-prod`, or pass `--profile <name>` to override."
4. **Guard write/deploy jobs against unintended remote targets.** When
   `infra jobs run` executes against a `remote`/cloud profile, require an
   explicit confirmation or `--profile`/`--yes` flag (especially for
   `publish_pipeline`, `copy_extension_assets`, deploy-class jobs). Local
   filesystem jobs should default to / strongly prefer the local profile.
5. **Make local the safe default for dev recipes.** Either pin `--profile local`
   in `just` recipes, or have the CLI detect a dev working tree and default
   accordingly.

## Repro (minimal)

1. Have two profiles (`local`, a remote `prod`); make `prod` active.
2. Let all session tokens expire (or revoke them).
3. `systemprompt admin session switch local` → observe misleading error + exit 0.
4. `just publish` → observe it targeting the active prod profile rather than local.
5. Re-run any job with `--profile local` → succeeds, confirming the switch path
   is unnecessarily strict.
