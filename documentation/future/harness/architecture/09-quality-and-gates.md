# 09. Quality and gates

The device code meets core's standards because it *is* core. The extension meets them through the install-side lint. There is no third copy of the rules.

## 1. One source for the gates

| Code | Judged by |
|---|---|
| Core device crates | Core's `scripts/check-gates.txt` (54 gates today), on the same push run as the rest of core |
| `extensions/harness/*` in the template and installs | The install-side lint the `future` design plans, consumed by core version |
| The harness repository's 29 forked scripts | Deleted with the repository |
| The bridge's 13 `lint-bridge-*` scripts | Folded in or retired (section 3) |

Gates that cannot apply to device crates (`check-sqlx`, `lint-migrations`, `lint-extensions`, `lint-repo-construction`) are scoped by path in one list with the reason. An absent gate is a decision, not an omission.

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
| Closure | Server kernel names no device crate; device crates name no server crate above `shared` | New, modelled on `lint-kernel-deps` |
| No window toolkit in core | No core crate depends on the toolkit | Harness `lint-no-window`, bridge `lint-bridge-no-window` |
| Platform confinement | `cfg(target_os)` only inside `platform` modules | New; replaces the stub convention |
| Host ids, headers, paths spelled once | No literal host id, header name or route outside its one module | Harness `lint-host-ids`, `lint-header-literals`, `lint-path-literals` |
| Foreign names | Names of other projects absent from files and paths | Harness `lint-foreign-names` |
| Contributions in `src/extension.rs` | Same convention as server members | Core |
| Verdict codes | Typed verdicts, no free strings | Bridge `lint-bridge-verdicts` |
| Native tests database-free | Native-matrix test crates do not reach `sqlx` or the fixtures | Core `lint-native-test-deps` (renamed scope) |
| Compositions | Every named composition builds | Core `scripts/build-compositions.txt`, extended |

Retired with the web tree: CSS tokens, i18n, JS imports, TypeScript bindings.

## 4. Test suites

| Suite | What it proves | Runs |
|---|---|---|
| **Conformance** | The wire and on-disk contracts in `05-bridge-split.md` section 4: manifests, signatures, acceptance cases, scoped tokens, heartbeat bodies, every host's written profile, command exit codes | Core push run. Recorded from the bridge *before* any module moves; every later stage must leave it green |
| **Unit and integration** | Each device crate's behaviour through its public API | Core test workspace; the moved bridge and harness tests |
| **Mock instance** | Sign-in, sync, a governed call on each protocol, a denied tool, heartbeat, telemetry rows, against an in-process instance | Core push run, Linux; scoped native. Starts from `crates/tests/mock-inference` |
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

In the `future` build report, per named composition: crate closure, binary size, cold and warm build time, gate duration. Baseline to record in S0: the bridge binary and the harness binary as they are today. A member that doubles a closure is visible in review.

## 7. What local work runs

Unchanged from core: check what you touched (`cargo clippy -p <crate> --all-targets --keep-going` and the crate's tests); the push run is the proof. Lanes run no tests, clippy or builds; the orchestrator validates once per round. Never the full shard set locally.
