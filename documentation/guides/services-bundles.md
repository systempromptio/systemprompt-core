# Publishing services as a bundle

How to package a services tree as a signed, versioned bundle, publish it to any HTTPS or OCI location, and have instances fetch, verify and compose it at boot.

By default an instance serves the services tree baked into its image at `paths.services`. Listing bundle sources in the profile makes that tree a composition of independently published artifacts instead, so changing a marketplace or an access rule becomes a publish and a restart rather than an image rebuild and a redeploy.

Bundles are host-agnostic. Nothing here assumes a particular registry, orchestrator or persistent volume.

## Prerequisites

- A services tree that loads: `config/config.yaml` plus the resource directories beneath it.
- Somewhere to publish: any HTTPS URL that serves the file, or any OCI registry.
- An ed25519 signing keypair, or a published SHA-256 digest to pin against. An instance refuses a non-bundled source that has neither.

## 1. What a bundle is

A bundle is a gzipped tar holding `bundle.json` and a `services/` subtree. `bundle.json` is a `SignedBundleManifest`: the manifest plus an optional detached signature.

| Manifest field | Meaning |
|---|---|
| `format` | Bundle format version. Currently `1`; an instance refuses anything else. |
| `version` | The version tag you stamped at pack time. |
| `created_at` | Pack timestamp, RFC 3339. |
| `requires_core` | A semver requirement such as `">=0.50"`. An older core refuses the bundle at fetch with a clear error rather than failing later on a key it does not understand. |
| `source` | Provenance: `repo`, `commit`, `workflow_run`. All optional, all free text. |
| `files` | Every file in the tree, each with its path, `checksum` and `size`. |
| `content_hash` | SHA-256 over the sorted `path\0checksum` lines. This is the bundle's identity: it keys the cache and the authz reconcile. |
| `total_size` | Sum of the file sizes. |
| `owns` | What this bundle claims, by id: `marketplaces`, `plugins`, `skills`, `rules`, `hooks`, `artifacts`, and the top-level `dirs`. Derived from the tree at pack time, never hand-written. |

The signature block carries `alg` (always `ed25519`), `key_id`, and `sig_b64`. It signs the canonical form of the manifest, so it covers every file digest transitively.

Verification runs in the order the trust chain requires, and every step is fatal:

1. The archive digest, if the profile pins one, before a byte is parsed.
2. The manifest signature, before the manifest is believed.
3. Per-file checksums, after extraction.
4. The `content_hash` against the recomputed file list.

There is no warn-and-continue path. A bundle that fails any step is never installed and never cached.

## 2. Base bundles and marketplace bundles

A bundle is one of two shapes, decided by what it owns.

A **base bundle** carries the platform-defined tree: `access-control`, `agents`, `ai`, `config`, `content`, `external_agents`, `gateway`, `governance`, `mcp`, `scheduler`, `slack`, `web`. Exactly one source in a profile may be a base bundle, and it must be the first.

A **marketplace bundle** carries only `marketplaces`, `plugins`, `skills`, `rules`, `hooks`, `artifacts` — the directories authored in Claude Code format and produced by [marketplace-authoring.md](marketplace-authoring.md). Every source after the first must be marketplace-only, and packing refuses to put a base directory in one.

That split is what lets several teams maintain their own marketplace repositories against one platform tree, each publishing on its own cadence.

## 3. Configure the profile

```yaml
services:
  cache_dir: /app/.cache/services      # default: <paths.system>/services-cache
  on_fetch_failure: use_last_good      # fail_closed | use_last_good | use_bundled
  sources:
    - name: base
      oci:
        reference: registry.example.com/org/base@sha256:<64 hex>
        auth_secret: registry_token
        verify:
          ed25519_public_keys: ["<base64 32 bytes>"]
    - name: sales-uk
      https:
        url: https://releases.example.com/sales-uk-2.1.0.tar.gz
        verify:
          sha256: "<64 hex>"
```

An empty or absent `sources` means the baked tree, which is the default and unchanged behaviour.

Each source names exactly one transport. `https:` takes a `url`; `oci:` takes a `reference`. Naming both, or neither, is a profile error. `name` must be unique: it is the cache key and the label in every status report and log line.

`auth_secret` names a key in the secrets document, never a value. For HTTPS it becomes a bearer token; for OCI it is the registry credential.

`verify` needs at least one of `sha256` (pin the exact archive) or `ed25519_public_keys` (trust a publisher). Pinning a digest is strongest and makes rollback exact; trusting a key lets you move a tag without touching the profile. A source with an empty `verify` is refused.

`cache_dir` must be writable and, on a cloud target, must sit under `/app`.

### Failure policy

`on_fetch_failure` decides what happens when a source cannot be fetched or fails verification. Every path logs at error level with the underlying failure text, and the choice is visible afterwards through the provenance in the status endpoint, so an instance serving stale content never looks healthy.

| Policy | Behaviour |
|---|---|
| `use_last_good` (default) | Serve the last successfully composed tree from the cache. If the cache is empty, fall back to the baked tree. |
| `use_bundled` | Serve the baked tree at `paths.services`. |
| `fail_closed` | Refuse to boot. |

`use_last_good` and `use_bundled` both fail the boot outright if there is neither a cached composition nor a baked tree with a `config/config.yaml` — there is nothing left to serve.

## 4. Composition and ownership

Sources compose in profile order into one root. Composition is **by id, not last-write-wins**: two bundles claiming the same marketplace, plugin, skill, rule, hook or artifact is a boot error naming both sources and the contested id.

That strictness is deliberate. Silently preferring one bundle would make which access rules an instance enforces depend on the order of a YAML list. Marketplace directories are shared by construction because their ids are disjoint; a base-only directory such as `access-control/` may have exactly one owner.

The composed root is content-addressed by the ordered list of member content hashes, so recomposing an unchanged set is a directory-exists check rather than a copy.

## 5. The cache

```
<cache_dir>/
├── bundles/<source-name>/<content-hash>/    # extracted, verified, per source
├── composed/<composed-hash>/                # the overlaid root
├── current -> composed/<composed-hash>      # symlink, swapped by rename
└── state.json
```

Everything is content-addressed, so re-fetching an unchanged bundle is a no-op and a rollback is a re-point rather than a download. `current` is swapped with `rename`, which is atomic on one filesystem: a reader sees either the whole previous composition or the whole new one, never a half-copied tree. `state.json` is written the same way.

`state.json` records `composed_hash`, `last_reconciled_hash` (the composition whose access rules were last projected into the database), and one entry per source with its `digest`, `version`, `content_hash` and `fetched_at`.

The cache keeps the two most recent versions per source and the two most recent composed roots, so a rollback to the previous version works offline.

## 6. How an instance picks up a new tree

A running instance does not poll. Nothing changes under it mid-request.

**At boot**, the source bootstrap runs before paths are resolved: fetch, verify, compose, swap `current`, record state. Every source is checked against `state.json` first, so an unchanged bundle costs one HEAD request.

**On demand**, two admin endpoints:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  https://api.example.com/api/v1/admin/services/status
```

`status` answers what tree the instance is actually serving and why. `provenance.kind` is `bundled`, `fetched`, or `last_good`, and carries `composed_hash` and, on a fallback, the `error` text that caused it. It also lists each source's digest, version, content hash and fetch time, plus `last_reconciled_hash`.

```bash
curl -X POST -H "Authorization: Bearer $TOKEN" \
  "https://api.example.com/api/v1/admin/services/refresh?restart=true"
```

`refresh` re-runs the whole resolution and answers `changed`, the new `composed_hash`, the per-source view, and `restarting`. The running process keeps its old root either way; `restart=true` asks the supervisor to bring the process back on the new composition, and only when something actually changed. Two refreshes cannot run at once: the second caller is refused with a conflict rather than queued behind a multi-megabyte download.

## 7. The CLI

```
systemprompt core services validate --root <ROOT> [--base <BASE>] [--against <AGAINST>] [--strict]
systemprompt core services keygen   [--out <OUT>]
systemprompt core services bundle   --root <ROOT> --out <OUT> --version <VERSION> \
                                    [--sign-key <path|env:VAR>] [--source-repo <S>] \
                                    [--source-commit <S>] [--workflow-run <S>] [--marketplace-only]
systemprompt core services publish  --bundle <BUNDLE> --to <TO> [--auth-secret <NAME>] [--auth env:VAR]
systemprompt core services refresh  [--check]
systemprompt core services inspect  [--bundle <BUNDLE>] [--active]
```

`validate` checks a tree before packing. `--base` resolves the tree's references — MCP server ids, agent ids — against a platform bundle, so a marketplace repository's pipeline can catch a dangling reference without access to the instance. `--against` compares with a previous bundle and, under `--strict`, errors when a plugin's files changed but its version did not.

`bundle` takes `--sign-key` as either a file holding the base64 seed or `env:VAR`, and `--marketplace-only` refuses to pack a base directory, which is what a marketplace repository's pipeline wants. `publish` takes its registry credential either from the profile's secrets by name (`--auth-secret`) or inline as `--auth env:VAR`. `inspect` reads an archive with `--bundle` or the instance's live composition with `--active`.

Output format is a global flag, not a per-command one: `--json` and `--yaml` go anywhere on the line.

`refresh` re-resolves the sources and, when the composition changes, runs the same per-bundle authz reconcile the boot path runs before reporting success. It is scriptable by exit code:

| Code | Meaning |
|---|---|
| `0` | Nothing changed. |
| `3` | The composition changed and was swapped. |
| `1` | Error. |

## 8. A publishing pipeline

The shape of a marketplace repository's CI job, with a generic registry:

```yaml
steps:
  - name: Import the authored tree
    run: systemprompt core marketplace import --from . --into ./services --strict

  - name: Validate against the platform bundle
    run: |
      systemprompt core services validate \
        --root ./services --base ./base-bundle.tar.gz --against ./previous.tar.gz --strict

  - name: Pack and sign
    run: |
      systemprompt core services bundle \
        --root ./services --out ./bundle.tar.gz \
        --version "${RELEASE_TAG}" --sign-key env:BUNDLE_SIGNING_SEED --marketplace-only \
        --source-repo "${REPO}" --source-commit "${COMMIT_SHA}" --workflow-run "${RUN_ID}"

  - name: Publish
    run: |
      systemprompt core services publish \
        --bundle ./bundle.tar.gz --to "oci://registry.example.com/org/sales-uk:${RELEASE_TAG}"

  - name: Roll the instance
    run: |
      curl -fsS -X POST -H "Authorization: Bearer ${ADMIN_TOKEN}" \
        "${API_URL}/api/v1/admin/services/refresh?restart=true"
```

The signing seed is a repository secret passed as `env:BUNDLE_SIGNING_SEED`, so it never lands on disk in the job. Instances hold only the public half, in `verify.ed25519_public_keys`.

The final step is optional. Omitting it means the new bundle is picked up at the instance's next restart, which is often what a scheduled deployment window wants.

## 9. Versioning

Four versions, each with one owner.

- **Content** — `plugin.json` `version` and `marketplace.json` `metadata.version`. Authored by whoever writes the marketplace.
- **Bundle** — `--version` at pack time, stamped into `bundle.json`. Pin by tag or by digest; digest is recommended for production because a tag can move.
- **Composed** — the hash of the ordered member hashes, computed by the instance. It keys the cache, the last-good root and the authz reconcile. Nobody authors it.
- **Format** — `format: 1` in the manifest and `requires_core` alongside it. An instance too old for a bundle says so at fetch.

Rolling back is re-pinning the previous tag or digest and restarting. The cache keeps the last two versions per source, so a rollback needs no network.

## Verify

```bash
systemprompt core services inspect --bundle ./bundle.tar.gz
systemprompt core services refresh --check
curl -H "Authorization: Bearer $TOKEN" https://api.example.com/api/v1/admin/services/status
```

`inspect --bundle` prints the manifest without installing anything, and `inspect --active` prints what the instance is serving. `refresh --check` reports whether the configured sources would change the composition, exiting `0` or `3`. `status` confirms what the instance is serving: a `provenance.kind` of `fetched` with no `error` means the tree came from the sources you configured.

## Related pages

- [marketplace-authoring.md](marketplace-authoring.md) — authoring the tree a marketplace bundle carries.
- [configure.md](configure.md) — the rest of the profile.
- [vault-secrets.md](vault-secrets.md) — the matching treatment for secrets.
- [../reference/configuration.md](../reference/configuration.md) — the `services:` key schema.
