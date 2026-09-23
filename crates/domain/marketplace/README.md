# systemprompt-marketplace

[![Crates.io](https://img.shields.io/crates/v/systemprompt-marketplace.svg?style=flat-square)](https://crates.io/crates/systemprompt-marketplace)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-marketplace?style=flat-square)](https://docs.rs/systemprompt-marketplace)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Loads marketplace catalogs, resolves enabled marketplace membership, applies per-user access rules and assembles signed bridge manifests and plugin bundles.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Usage

```toml
[dependencies]
systemprompt-marketplace = "0.60"
```

```rust,ignore
use systemprompt_marketplace::register_marketplace_filter;

register_marketplace_filter!(MyAclFilter::new, priority = 100);
```

## Public Surface

| Item | Description |
|------|-------------|
| `MarketplaceService` | Read-only resolution over a borrowed `ServicesConfig`: lookup, default fallback, active marketplace, referential-integrity check. |
| `ManifestService` / `CanonicalView` | Assemble a scoped, filtered `MarketplaceCandidate` and sign the canonical view. |
| `catalog::CatalogContent` / `plugin_bundles` | On-disk loaders projecting the services tree into the signed `*Entry` records; `plugin_bundles` is the single source of the active, content-gated plugin bundles shared by the manifest and serving paths. |
| `bundle` (`build_plugin_bundle`, `PluginBundle`, `BundleContent`, `BundleFile`) | Build-from-spec assembler that owns the `.claude-plugin` bundle contract. |
| `scope_to_marketplace` / `active_marketplace` | Marketplace scoping of the catalogue lists. |
| `view::render_marketplace_json` / `render_marketplace_list` | JSON projections for the HTTP catalogue endpoints. |
| `MarketplaceFilter` | Async trait implemented by ACL backends, applied before signing. |
| `MarketplaceCandidate` | Mutable bundle of `plugins`, `skills`, `agents`, `hooks`, `managed_mcp_servers`, and `artifacts` vectors plus optional `marketplace_id` and `access`, handed to the filter. |
| `AllowAllFilter` | Passthrough default returned when no extension registers a filter. |
| `MarketplaceError` / `MarketplaceFilterError` | Crate-wide error (lookup, catalogue load, signing) and the narrower filter error folded into it. |
| `MarketplaceFilterRegistration` / `discover_filters` | `inventory`-collected registration record with priority ordering, and its lookup (highest priority first). |
| `register_marketplace_filter!` | Compile-time registration macro for a filter factory. |

## Wiring

`AppContext` holds an `Arc<dyn MarketplaceFilter>`. At startup the runtime calls `discover_filters()`, picks the highest-priority registration, and falls back to `AllowAllFilter` when no registration is present or the factory returns an error. The factory receives a `&DbPool`.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Manifest entry types (`PluginEntry`, `SkillEntry`, `AgentEntry`, `HookEntry`, `ManagedMcpServer`). |
| `systemprompt-identifiers` | Typed identifiers. |
| `systemprompt-database` | `DbPool` passed to filter factories. |
| `systemprompt-security` | Ed25519 signing of the canonical manifest view. |
| `async-trait` | Async methods on the `dyn`-compatible `MarketplaceFilter`. |
| `inventory` | Compile-time filter registration. |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---


## Consumer installation evidence

Consumer evidence uses credentials scoped to an enrolled `user_device_certs` record.
The bridge obtains one by self-enrolling its device through the gateway
(`POST /v1/bridge/device`); the credential is returned once, stored server-side only
as a SHA-256 digest, and replaced by subsequent issuance. Certificate or credential
revocation (`ManagedRepository::revoke_consumer_credential`) prevents new receipt and
session mutations. A user JWT or certificate fingerprint is not a consumer evidence
credential.

The bridge confirms its credential with `POST /api/v1/consumer-devices/enrollment`
using `Authorization: Bearer <credential>`. The response identifies the enrolled
device and consumer.

`POST /api/v1/consumer/receipts` accepts the shared `ConsumerReceiptRequest`, compares
all files in the retained publication dependency bundle, and acknowledges identical
retries. File byte counts, digests, executable flags, revision, publication generation,
and bundle digest must match retained records. Explicit unavailable readback checks
remain unverified and cannot support a session binding.

`POST /api/v1/consumer/session-bindings` binds a verified receipt to the authenticated
consumer, device, host, and native session. Bindings of one native session are
serialized, and a repeated binding is acknowledged only when it is identical.

Consumer mutations reuse the current bridge catalog and per-user marketplace filter.
Only a filtered managed publication with the requested canonical resource identity
and enabled host is eligible. Successful catalog authorization retains a resource
grant; explicit administrative revocation (`ManagedRepository::set_consumer_grant`)
is never reset by later catalog reads. These four routes are the whole managed HTTP
surface; administration happens in-process through `ManagedRepository`.

Historical receipts keep their existing evidence and have no invented consumer or
device identity. Their session-shaped metadata cannot establish a consumer binding.

Installation coverage per resource is recomputed by the `managed_inventory_refresh`
job and written only when it changed.

## Git source synchronization

Git source synchronization uses HTTPS with source-scoped Bearer credentials resolved by the runtime `GitSourceOrchestrator`. The Git subprocess clears ambient environment/configuration and credential helpers, disables redirects and submodule recursion, and enforces 60-second subprocess and 120-second aggregate Git deadlines. Output is limited to 8 MiB, retained trees to 256 files/8 MiB, and temporary repository storage is monitored against 64 MiB. Temporary directories are private on Unix and removed before a result is returned; reader threads are joined and the Git child is reaped after process-group termination.

The test fixtures separate actual local TLS Git transport acceptance (authentication, rotation, redirect refusal and redacted errors) from an injected trusted capture for synchronization contracts.


Consumer installation plans are downloaded from
`GET /api/v1/consumer/resources/{resource}/publications/{publication}/bundle?host={client}`.
They derive active native entrypoint, supporting files and dependency targets from
the exact retained publication. Receipts carry separate `runtime_files` readback;
canonical source-cache evidence without the complete active runtime projection is
never fully verified. Host aliases preserve `codex-cli`/`codex` and
`opencode`/`open-code` compatibility. The retained resource owner supplies grant
ownership; changing the configured administrator does not invent a new publisher.
