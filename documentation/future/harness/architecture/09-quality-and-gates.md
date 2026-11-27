# 09. Quality and gates

The device code meets core's standards because it *is* core. The extension meets them through the install-side lint. There is no third copy of the rules.

## 1. One source for the gates

| Code | Judged by |
|---|---|
| Core device crates | Core's `scripts/check-gates.txt` (71 gates at the end of S0, the closure gate among them), on the same push run as the rest of core |
| `extensions/harness/*` in the template and installs | The install-side lint the `future` design plans, consumed by core version |
| The harness repository's 29 forked scripts | Deleted with the repository; the few with no core equivalent are merged into core's or the template's in S1 |
| The bridge's 13 `lint-bridge-*` scripts | Folded in or retired (section 3) |

### 1.1 Core gates that assume a server crate

No gate fires on an empty crate, so S0 scoped nothing. The decision falls to S1 wave C (S1-C2), when the lifted code first meets the gates, and again to S6 for the bridge's. The rule it applies:

- **A coding-standard gate is met.** The lifted code is changed until it passes. No device crate is scoped out of one, and no allowlist entry is added to get a move through.
- **A schema, Postgres or HTTP gate is scoped by path**, in the gate's own scope mechanism, with one written reason per gate. These gates encode that a crate is a server member with a Postgres schema or an HTTP surface, which a device crate is not by design.
- **An absent gate is a decision, not an omission.** The scope list is one reviewed file; a device crate missing from a gate without a line there is a defect.

Scoped by path for the eight device crates:

| Gate | Assumes | Reason it cannot apply |
|---|---|---|
| `lint-schema`, `lint-extensions`, `lint-migrations`, `lint-table-ownership` | Schema DDL and migrations owned by a server `Extension` and installed in Postgres | A device crate registers no server extension and owns no Postgres table. The engine's store is a local SQLite file with its own versioning |
| `lint-sqlx`, `sqlx-audit-caches`, `lint-typed-binds` | Compile-checked Postgres queries and their offline cache | Device crates do not link the Postgres driver; the closure gate proves it |
| `lint-repo-construction`, `lint-ledger-reads` | Repositories built over the server's database pool; reads of the server's ledgers | Neither exists on a device |
| `lint-domain-protocol-deps` | No domain crate depends non-optionally on an HTTP server or command-line framework | Scoped for `systemprompt-proxy` only, from S6: a loopback HTTP server is that crate's purpose. Every other device-domain crate meets it. If the engine's model transport turns out to need one of the banned crates non-optionally, that is reported, not scoped |

Met, with what meeting them takes:

| Gate | What the lifted code must do |
|---|---|
| `lint-owned-tasks` | Spawn only through the owned-task types in `systemprompt-traits`. The harness's own task crate is deleted; the operations its callers use and core lacks (a blocking spawn, an owned thread, stop, child tokens, a shutdown report) are added to `systemprompt-traits`, the one place the gate allows a spawn (D46) |
| `lint-no-inventory` | No link-time registration anywhere under `crates/`. The extension contract's registry becomes an explicit list in S1 (D46) |
| `lint-env-vars` | Every environment read is listed in the owning package's allowlist with its reason, or becomes a setting. This is the gate's own mechanism, not a scope |
| `lint-raw-ids` | Typed ids everywhere. The gate derives its names from `systemprompt-identifiers`, so each id added there switches the gate on for that name across all of core; a name already used loosely in server code (job ids are the likely one) produces findings in crates the lift never touched, and those are fixed in the same wave |
| `lint-capability-cfg` | A `cfg(feature = …)` that names a capability sits on a `mod` or `use`. Applies to the per-product features of `systemprompt-hosts` and to `lockdown` once the facade forwards them |
| `lint-host-ids`, `lint-header-literals`, `lint-path-literals` | Host ids, header names and routes are spelled once, in the shared crates. A route or header a device needs that has no constant gets one there first (`../testing/02-scripted-instance.md` section 2 lists the ones known missing) |
| `lint-unbounded-bodies`, `lint-json-value`, `lint-stringly-errors`, `lint-async-trait`, `lint-test-seams`, `lint-inline-tests`, `lint-comments`, `check-headers`, `lint-serde-aliases`, `lint-core-globals`, `lint-state-compare`, `lint-mode-enums`, the four `rust-contracts` gates | Nothing special: they scan all of `crates/` and are listed because the lifted volume will trip them |

Not a gate, same rule: the workspace denies `unsafe_code`. Platform modules in `systemprompt-device` and `systemprompt-machine` (keyrings, access-control lists, elevation) will meet it; how they are allowed is decided in the wave that lands the first one and recorded here.

Device crates are in the unit table as one unit, `capability:device`. That name joins the capability names `lint-capability-cfg` polices.

## 2. Standards that apply unchanged

From core's `CLAUDE.md` and the `rust-coding-standards` skill, to device crates and to the extension alike:

- Typed identifiers through `systemprompt-identifiers`; construct with `new`, `try_new`, `generate`.
- `thiserror` enums in libraries; `anyhow` only in the binary and tests. An error built from another keeps it as a source. No stringly errors.
- Native `async fn`; `#[async_trait]` only for `dyn`, documented on the trait. (`HostSync` and `Governor` are `dyn` and say so.)
- Owned background work through `BackgroundTasks` or `OwnedTask`; child processes supervised and signalled only when provably ours.
- `tracing` with structured fields; no `println!` in libraries.
- No inline WHAT comments; `// Why:` only for an externally imposed constraint; rustdoc on traits, top-level types and modules only.
- No legacy code, no shims, no dual paths. One compiled shape: no test features or seams in production crates.
- No environment readers outside the allowlist. The bridge's `SP_BRIDGE_*` readers are each listed with a reason or become settings.
- Fail-closed policy expansion.
- No globals. The bridge's `brand()` `OnceLock` and the `lint-bridge-globals` allowlist entries are removed by the conversion to composition state.
- Tests only in the separate test workspace, reaching public API.
- Bridge correctness rules: logging does not turn a failed operation into success; verified writes; exact-scope reads; gateway-bound trust; typed partial outcomes. `lint-discarded-results` and `lint-fail-open` run on every device crate.

## 3. Gates added or moved

| Gate | Enforces | Origin |
|---|---|---|
| Layers, extended | Device crates obey the five layers; device-domain peers do not depend on each other | `lint-layers`, `lint-bridge-layers` |
| Closure | Server kernel names no device crate; device crates name no server crate above `shared`; no device crate is a kernel unit; all eight are workspace members | `lint-device-closure`, added in S0, modelled on `lint-kernel-deps`, with its own self-test |
| No window toolkit in core | No core crate depends on the toolkit | Harness `lint-no-window`, bridge `lint-bridge-no-window` |
| Platform confinement | `cfg(target_os)` only inside `platform` modules | New; replaces the stub convention |
| Host ids, headers, paths spelled once | No literal host id, header name or route outside its one module | Harness `lint-host-ids`, `lint-header-literals`, `lint-path-literals` |
| Foreign names | Names of other projects absent from files and paths | Harness `lint-foreign-names` |
| Contributions in `src/extension.rs` | Same convention as server members | Core |
| Verdict codes | Typed verdicts, no free strings | Bridge `lint-bridge-verdicts` |
| Native tests database-free | Native-matrix test crates do not reach `sqlx` or the fixtures. The scripted-instance crate is one of them (D48) | Core `lint-native-test-deps` (renamed scope) |
| Compositions | Every named composition builds | Core `scripts/build-compositions.txt`, extended |

Retired with the web tree: CSS tokens, i18n, JS imports, TypeScript bindings.

## 4. Test suites

| Suite | What it proves | Runs |
|---|---|---|
| **Conformance** | The wire and on-disk contracts in `05-bridge-split.md` section 4: manifests, signatures, acceptance cases, scoped tokens, heartbeat bodies, every host's written profile, command exit codes | Core push run. Recorded from the bridge *before* any module moves; every later stage must leave it green |
| **Unit and integration** | Each device crate's behaviour through its public API | Core test workspace; the moved bridge and harness tests |
| **Mock instance** | Sign-in, sync, a governed call on each protocol, a denied tool, heartbeat, telemetry rows, against an in-process instance | Core push run, Linux; scoped native. The instance is the scripted-instance crate (`../testing/02-scripted-instance.md`), a new crate beside `crates/tests/mock-inference`, not a change to it |
| **Composition boot** | Every form in `02-target-architecture.md` section 5 builds, admits and starts its role | Core for windowless forms; template for forms with a window |
| **Fork contract** | On a fork's own composition: admission succeeds; no reserved shape is held by an extension member; every declared tool has a schema; a denied tool does not run; a governor layer cannot widen; a host integration's profile points only at the proxy and holds a scoped token | Template and every install |
| **Property** | The verdict lattice under layers; acceptance ordering; scoped-token domain separation | Core |
| **View parity** | Each bridge fixture state (`web/dev/fixtures/*.json`) has a native view that renders it | Template |
| **Smoke** | The packaged app launches and reaches the sign-in gate | Template and installs, three systems |
| **Manual walk** | `08-platforms-and-packaging.md` section 6 | Before the fleet handover |

Rules carried from the `future` programme: a test that never executes its body is a defect (three suites once shipped green that way); a flake seen in a release is fixed at its root; a red push run on `next` or `future` is the next thing to fix.

## 5. Security review

An independent, read-only review lane, separate from the implementing lane, for each of:

1. The bridge split (credentials, trust, the policy writer cross a crate boundary).
2. The device composition's reserved-shape check and admission.
3. `GovernorLayer`.
4. Extension hosts and `HostId` on the authorization path.
5. `harness.members`.
6. The fleet handover's import of secrets and rewrite of host profiles.

On the `future` programme such a lane caught a scope-widening regression the implementing lane had reported as safe.

## 6. Measurement

In the `future` build report, per named composition: crate closure, binary size, cold and warm build time, gate duration. Baseline: the bridge binary and the harness binary as they are before the lift. No S0 lane built either, so it is still to be recorded; S1's stage gate is the last point at which the unlifted harness can be measured. A member that doubles a closure is visible in review.

## 7. What local work runs

Unchanged from core: check what you touched (`cargo clippy -p <crate> --all-targets --keep-going` and the crate's tests); the push run is the proof. Lanes run no tests, clippy or builds; the orchestrator validates once per round. Never the full shard set locally.
