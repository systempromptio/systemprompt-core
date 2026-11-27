# 02. Target architecture

## 1. Two tiers

| Tier | Holds | Links the window toolkit |
|---|---|---|
| **Core** (`systemprompt-core`, published crates) | The wire contract and host identity. The device composition machinery. Every governed device capability as an opt-in facade feature | No |
| **The extension** (template, and each install beside its web and MCP extensions) | A one-line `main` and its composition. Window, views, themes, strings. Brand and packaging. The product's own members | Yes, when the product has a window |

The rule that draws the line: **core owns everything that carries trust or that the server also runs; the extension owns the product surface. Nothing in the extension can widen what core enforces.**

This mirrors the server side. Core holds the domains and their rules; the template's `extensions/web` holds the admin and site surface, and installs fork it.

## 2. A harness has two halves

- **Server half.** A core member (capability `harness`) in the install's server binary. It registers the host, its protocols and marketplace settings, publishes `harness.members`, and later owns routes for tasks, routines and remote control. It implements core's existing `Extension`.
- **Device half.** The binary on a user's machine or a server. It is composed from `HarnessExtension` members (`03-composition-contract.md`). It does not implement the server trait, whose sink is schemas, migrations, routes and jobs.

Where a capability has a device presence (Drive, Brain, issues, agent runs, notifications), its unit gains a third surface beside `http` and `cli`.

## 3. Where device crates sit in core

Decided: eight new crates. Core's five layers hold, with device crates as peers that the server kernel never names.

| Layer | Crate | Holds |
|---|---|---|
| shared | existing `models::bridge`, `identifiers`, `wire`, `client`, `traits` | Wire types, host identity, extension-host descriptors, typed ids (the engine's, where a name is already taken, in an `identifiers::engine` submodule), typed instance calls, owned tasks (with the additions the lifted callers need) |
| shared | existing `provider-contracts` | The composition machinery shared with the server: slots, standing, typed state, admission |
| infra | `systemprompt-device` | Sign-in, keyrings, config and policy stores, manifest fetch, verification, acceptance, apply, ledgers, platform modules |
| infra | `systemprompt-machine` | Install, device-management snippets, policy writer, elevation, scheduler, ACLs; `lockdown` as a feature |
| domain | `systemprompt-engine` | Agent loop, tools, MCP, LSP, workspace, store, configuration, prompts, model transport |
| domain | `systemprompt-proxy` | Loopback proxy, loopback secret, scoped tokens, heartbeat, usage |
| domain | `systemprompt-hosts` | The host contract; one feature per product: `claude`, `codex`, `opencode`, `hermes` |
| domain | `systemprompt-drive` | Folder sync |
| domain | `systemprompt-runner` | Headless adapters, shared with the server's agent runs (see "The runner and the server" below) |
| app | `systemprompt-harness` | The device composition, the member contract and sink, the governor, projection, update, commands, shared client state machines |
| entry | none in core | The binary is the extension's |

Constraints:

- The layer gate extends to these crates with an empty allowlist. Device-domain crates are peers: they meet through slots and shared traits, never by depending on each other.
- A closure gate: server kernel crates name no device crate, and device crates name no server crate above `shared`. The two closures meet only in `shared`.
- Inside `systemprompt-engine`, a module-layer gate keeps the order `config`, `store`, `llm`, `tools`, `mcp`, `workspace`, `lsp`, `prompt`, `run`.
- All eight are published with core.

State at the end of S0: the eight crates exist, empty, as root-workspace members in one unit (`capability:device`), which is what lets `systemprompt-harness` depend on the other seven under the capability gate. The layer gate holds each at its layer and the closure gate (`lint-device-closure`) is in the gate list. The closure gate's four rules: no crate in the kernel closure names a device crate (the facade is excluded from the kernel side, since its features select the slices); no crate in the device closure names anything but a device crate or a `shared` crate; no device crate is a kernel unit; all eight are workspace members, so the gate cannot pass because its subjects were renamed away. The engine's module-order gate is written in S1 wave C. The facade gains no feature for these crates until S5.

### Placement at the lift

The table above is the target. S1 lands the harness repository's code before the bridge's, and some of it cannot sit where the target puts its successor (D46):

| Lifted code | Lands in S1 | Why not the target crate |
|---|---|---|
| The harness's sync | `systemprompt-harness` | It depends on governance, projection and the extension contract, which are app-layer; `systemprompt-device` is infra. S6 replaces it with the bridge's sync, in `device` |
| The extension contract and its registration | `systemprompt-harness`, with an explicit registration list | Core refuses link-time registration (`lint-no-inventory`); S5 replaces the list with the composition |
| The Drive's engine half (its tools and agent) | `systemprompt-harness` | It depends on the engine; device-domain crates are peers |
| Brain, notifications, Slack (device members) | `systemprompt-harness` | Members of the device composition, not server domain code |

Numbers and the full tables are in `06-harness-repo-disposition.md` and `../stages/s1-lift.md`.

### The runner and the server

`systemprompt-runner` is to be linked by the device composition and by the server's `systemprompt-agent-runs`. As the graph stands at S0 two gates refuse that, and the skeleton stage found it before any code moved:

1. **Closure.** `systemprompt-runtime` is a kernel crate and depends on `systemprompt-agent-runs`, for one type (the admitted run origins). An `agent-runs → runner` edge would therefore put a device crate in the kernel closure; the closure gate's own self-test pins that case as a failure. Decided (D49): the `runtime → agent-runs` edge goes. The type moves to the shared-layer contract through which the kernel already reaches members, and the kernel names no capability crate. The closure gate gets no exception.
2. **Capability.** `agent-runs` is unit `capability:agents` and the runner is `capability:device`; `lint-capability-deps` refuses a dependency between two capabilities. Removing the kernel edge does not answer this. Open (O5 in `../risks-and-decisions.md`), needed by S9: a unit for the runner that both sides may link, with the rule that it depends on nothing above `shared`, or the crate moves to the shared layer.

Both are entry work for S9-A3 (`../stages/s9-server-and-orchestration.md`). Until then `agent-runs` keeps its own adapters and names no device crate.

## 4. Slices

Each kernel member is a facade feature of `systemprompt`, so a build links only what it names.

| Slice | Brings | Depends on | Standing |
|---|---|---|---|
| `device` | Sign-in, key trust, sync, heartbeat | Always present | Core, required |
| `governor` | Model-call signing, verdicts, telemetry, model list | `device` | Core, reserved |
| `engine` | The one runtime: loop and tools, run in a typed mode (`11-execution-modes.md`): `Unrestricted` or `Sandboxed` on this device, or `Remote` on the instance | `governor` | Core, optional |
| `proxy` | Loopback proxy, loopback secret, scoped tokens | `device` | Core, reserved |
| `host-claude`, `host-codex`, `host-opencode`, `host-hermes` | One host integration each (features of `systemprompt-hosts`) | `proxy` | Core, optional |
| `machine` | Install, MDM, policy writer, drift repair | `device` | Core, optional |
| `lockdown` | Enforced machine policy | `machine` | Core, optional |
| `runner` | Start and observe sessions in other agents | The host slices it drives | Core, optional |
| `drive`, `update` | Folder sync; self-update | `device` | Core, optional |
| `terminal`, `preview` | The integrated terminal; the preview pane, which is the only place a web view exists (`12-workbench-and-automation.md`) | `engine` | Core and extension, optional |
| `schedules`, `memory` | Scheduled `Remote` sessions; recall and learning through core's memory capability | `engine` | Core, optional |
| `client` and its feature slices (`chat`, `runs`, `approvals`, `inbox`, `documents`) | Client state machines for everything that runs on the server: session stream, approval queue, inbox, documents. Shared with the phone apps ([mobile set](../../mobile/architecture/02-target-architecture.md)) | `device` | Core, optional, each opt-in |
| `shell`, `tray` | Window and views; background presence | `device` | Extension |

## 5. Every form it must take

Each row is a deployment and nothing but a list of slices. Each is a named composition, built and booted in CI against the mock instance (`09-quality-and-gates.md`).

| Form | Slices | Window |
|---|---|---|
| Govern Claude Code and Claude Desktop | `device`, `proxy`, `host-claude`, `machine`, `tray` | Tray |
| Govern Codex | `device`, `proxy`, `host-codex`, `tray` | Tray |
| Govern OpenCode | `device`, `proxy`, `host-opencode`, `tray` | Tray |
| The bridge: no harness, manage coding agents only | `device`, `proxy`, every `host-*`, `machine`, `tray`, `update` | Tray or none. This is today's bridge |
| Headless on a server or in CI | `device`, `proxy`, chosen `host-*` | None (`serve`) |
| Reference harness | `device`, `governor`, `engine`, the client feature slices, `shell`, `drive`, `update` | Full |
| Harness and bridge together | The two rows above combined | Full, with a hosts page |
| Your own harness | Reference slices plus a fork's members | Full, the fork's brand and theme |
| Orchestrator above the agents | Harness and bridge plus `runner` | Full |
| Locked-down machine | Any row plus `machine` and `lockdown` | As the row |
| `Remote` sessions | The same runtime's headless role, started by the instance on a runner node in its sandbox | None |

### Lockdown, and its limit

Built on what the bridge enforces today: trust pinned in the machine policy location, the SYSTEM policy writer, a profile no user account can rewrite, drift repair. The slice adds: a required-members policy, refusal to run when the pinned instance is unreachable longer than policy allows, and managed settings for each agent that pin it to the proxy.

It governs the agents it manages and reports those it finds unmanaged. It does not stop a user with administrator rights from installing another tool. Blocking that is device management; `machine` emits the MDM snippets for it. This limit is stated in the product and in the external guide.

## 6. Roles of the one binary

| Role | Started by | Uses |
|---|---|---|
| `desktop` (default when launched without a console) | The user | `shell` |
| `serve` | Login item, service, or command line | No window; proxy and sync |
| `run` | Command line, or the instance for a `Remote` session | The runtime, headless, one session, in the mode named |
| Declared commands | Command line, other agents, schedulers | `credential-helper`, `sync`, `install`, `doctor`, `login`, … |

A bare invocation with no subcommand and a console is the credential helper, as the bridge behaves today; other agents' configurations depend on it.

## 7. Background process

Third-party agents depend on the proxy and on sync whether or not a window is open. Any composition with `proxy` runs in the background, starts at login, and closing the window does not stop it. Single-instance and hand-off to a running process exist in both codebases; the bridge's implementation is the one kept.

## 8. What does not integrate

The device binary does not link the server runtime, the database layer or `AppContext`. There is no embedded local instance: core is Postgres-only, its kernel floor is about 810 crates, and a device that carried its own instance would lose the property that every call is governed by a real one.
