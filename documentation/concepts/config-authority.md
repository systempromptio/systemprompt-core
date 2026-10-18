# Config planes

A config plane is a configuration file under the services tree that is projected into database rows. A typical example is an access-control file whose rules become rows the runtime enforces. Extensions contribute planes through `Extension::config_planes()`. The host reconciles each plane against its file, records every change it makes, and exposes the result in `/readyz`, in the Prometheus metrics and through `admin config planes`.

The contract is `ConfigPlane` in `crates/shared/provider-contracts/src/config_plane/`. The reconciler is `ConfigReconcileService` in `crates/app/scheduler/src/services/config_reconcile/`.

## Authority

Each plane runs under one authority. The profile sets it under `config_planes.authority`:

```yaml
config_planes:
  authority:
    access_control: db   # the console owns the rows; the file only seeds them
    # every plane not listed is `file`
```

| Effective authority | When | What the reconciler does |
|---|---|---|
| `file` | The profile says `file` (the default) and the declaration file exists. | Overwrites any database drift back to the file, at boot and on every `config_reconcile` run (each minute). Console writers are refused. |
| `db` | The profile says `db`. | Seeds the plane from its file while the projection is empty, with mode `seed_if_empty` and actor `seed`. After that, drift is only reported and never written. |
| `undeclared` | The profile says `file` but the file is absent. | Never writes. A missing file never empties a table. |

Two more rules protect live data. An empty declaration under `file` authority never deletes every row unattended; the reconciler fails that plane instead. An apply that does not leave the database equal to the declaration fails and is rolled back.

## Ordering and effect

A plane can name the planes whose rows it references through `depends_on()`. The host orders planes so each one follows its dependencies, using registration order to break ties, and reconciles them in that order. At boot it refuses to start when a plane depends on an unregistered plane or when the dependencies form a cycle.

A plane's `effect()` says when a written change reaches the running system. `live` means the next read sees it. `on_restart` means the runtime loads the projection at boot, so a change takes effect only after a restart. The effect appears in `admin config planes list` and `diff`, and in each plane entry of the `/readyz` `config` block.

## Audit and atomicity

Each plane's read, apply and audit run in one database transaction. The host passes the transaction to the plane as a `PlaneTx`, and the plane recovers the driver handle with `PlaneTx::get`. After the apply, the host reads the plane again on the same transaction. It writes one `config_change_audit` row for each entity whose value actually changed, with the before value, the after value, the mode, the actor and the reason, and then commits once. A change and its audit therefore commit or roll back together.

`config_plane_state` holds one row per plane with the last authority, declaration hash, remaining drift and outcome. `/readyz` and `admin config planes list` read it, so every replica reports the same state whichever one ran the pass.

## Writing a plane's rows from a console: the writer token

An extension that edits a plane's rows directly (an admin page or an API route) must hold a `PlaneWriter` for that plane. The only way to get one is `ConfigPlaneSet::writer(name)`, available as `ExtensionContext::config_planes()?.writer(name)?`. `PlaneWriter` has no public constructor, so a write path that takes `&PlaneWriter` cannot run without that check.

`writer()` refuses a plane under effective `file` authority with `ConfigPlaneError::FileManaged`. Converted to an HTTP error, this becomes `409 Conflict` with error key `config_plane_file_managed`, because the reconciler would overwrite the edit on its next pass. An unknown plane is `404`. A plane under `db` or `undeclared` authority gets a writer.

## Optional capabilities

| Method | Default | Purpose |
|---|---|---|
| `export(ctx, tx)` | Refuses with `ConfigPlaneError::Unsupported` | Renders the current rows as declaration text. `Ok(None)` means there is nothing to export. |
| `supports_scoped_apply()` | `false` | When `true`, the plane honours `EntityScope::Entities` in `ApplyRequest`. The host refuses a scoped apply to any other plane. |
| `depends_on()` | `[]` | See [Ordering and effect](#ordering-and-effect). |
| `effect()` | `live` | See [Ordering and effect](#ordering-and-effect). |

## Operator commands

```bash
systemprompt admin config planes list
systemprompt admin config planes diff --plane access_control
systemprompt admin config planes export --plane access_control --out services/access/rules.yaml [--force]
systemprompt admin config planes apply --plane access_control --reason "OPS-123" \
    [--mode overwrite|insert-only|seed-if-empty] [--entity <key> ...]
```

`apply` takes the cluster-wide reconcile lock and records actor `cli` with the given reason. `--mode seed-if-empty` does nothing when the plane already has rows. `--entity` limits the apply, and its audit, to the named entity keys.

## Runbook: moving a plane from `db` to `file` authority

Under `file` authority the declaration replaces whatever the console wrote. Rows that exist only in the database are deleted on the first pass. Capture them first:

1. Leave the plane at `db` authority and run `admin config planes export --plane <name> --out <declared path>` against the production database.
2. Review the exported file, commit it to the services tree, and deploy the tree.
3. Run `admin config planes diff --plane <name>` and confirm it reports no drift rows.
4. Remove the plane's `db` entry from `config_planes.authority` (or set it to `file`) and roll out the profile.
5. The next pass reports `in_sync`. Console writers for the plane now receive `409 config_plane_file_managed`.

A plane whose `export` is unsupported cannot be captured this way. Write its declaration by hand from `diff`, which lists every `extra` row the file would delete, and repeat step 3 until no drift remains.
