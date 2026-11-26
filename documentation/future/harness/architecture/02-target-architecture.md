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
| shared | existing `models::bridge`, `identifiers`, `wire`, `client`, `traits` | Wire types, host identity, extension-host descriptors, typed ids, typed instance calls, owned tasks |
| shared | existing `provider-contracts` | The composition machinery shared with the server: slots, standing, typed state, admission |
| infra | `systemprompt-device` | Sign-in, keyrings, config and policy stores, manifest fetch, verification, acceptance, apply, ledgers, platform modules |
| infra | `systemprompt-machine` | Install, device-management snippets, policy writer, elevation, scheduler, ACLs; `lockdown` as a feature |
| domain | `systemprompt-engine` | Agent loop, tools, MCP, LSP, workspace, store, configuration, prompts, model transport |
| domain | `systemprompt-proxy` | Loopback proxy, loopback secret, scoped tokens, heartbeat, usage |
| domain | `systemprompt-hosts` | The host contract; one feature per product: `claude`, `codex`, `opencode`, `hermes` |
| domain | `systemprompt-drive` | Folder sync |
| domain | `systemprompt-runner` | Headless adapters, shared with the server's agent runs |
| app | `systemprompt-harness` | The device composition, the member contract and sink, the governor, projection, update, commands |
| entry | none in core | The binary is the extension's |

Constraints:

- The layer gate extends to these crates with an empty allowlist. Device-domain crates are peers: they meet through slots and shared traits, never by depending on each other.
- A closure gate: server kernel crates name no device crate, and device crates name no server crate above `shared`. The two closures meet only in `shared`.
- Inside `systemprompt-engine`, a module-layer gate keeps the order `config`, `store`, `llm`, `tools`, `mcp`, `workspace`, `lsp`, `prompt`, `run`.
- All eight are published with core.

## 4. Slices

Each kernel member is a facade feature of `systemprompt`, so a build links only what it names.

| Slice | Brings | Depends on | Standing |
|---|---|---|---|
| `device` | Sign-in, key trust, sync, heartbeat | Always present | Core, required |
| `governor` | Model-call signing, verdicts, telemetry, model list | `device` | Core, reserved |
| `engine` | Native agent loop, tools, MCP, LSP, store | `governor` | Core, optional |
| `proxy` | Loopback proxy, loopback secret, scoped tokens | `device` | Core, reserved |
| `host-claude`, `host-codex`, `host-opencode`, `host-hermes` | One host integration each (features of `systemprompt-hosts`) | `proxy` | Core, optional |
| `machine` | Install, MDM, policy writer, drift repair | `device` | Core, optional |
| `lockdown` | Enforced machine policy | `machine` | Core, optional |
| `runner` | Start and observe sessions in other agents | The host slices it drives | Core, optional |
| `drive`, `update` | Folder sync; self-update | `device` | Core, optional |
| `client` and its feature slices (`chat`, `runs`, `approvals`, `inbox`, `documents`) | Screen models for everything that runs on the server: state, streams, actions. Shared with the phone apps ([mobile set](../../mobile/architecture/02-target-architecture.md)) | `device` | Core, optional, each opt-in |
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
| Reference harness | `device`, `governor`, `engine`, `shell`, `drive`, `update` | Full |
| Harness and bridge together | The two rows above combined | Full, with a hosts page |
| Your own harness | Reference slices plus a fork's members | Full, the fork's brand and theme |
| Orchestrator above the agents | Harness and bridge plus `runner` | Full |
| Locked-down machine | Any row plus `machine` and `lockdown` | As the row |
| Server-side agent runs | `device`, `governor`, `engine` | None (`run`) |

### Lockdown, and its limit

Built on what the bridge enforces today: trust pinned in the machine policy location, the SYSTEM policy writer, a profile no user account can rewrite, drift repair. The slice adds: a required-members policy, refusal to run when the pinned instance is unreachable longer than policy allows, and managed settings for each agent that pin it to the proxy.

It governs the agents it manages and reports those it finds unmanaged. It does not stop a user with administrator rights from installing another tool. Blocking that is device management; `machine` emits the MDM snippets for it. This limit is stated in the product and in the external guide.

## 6. Roles of the one binary

| Role | Started by | Uses |
|---|---|---|
| `desktop` (default when launched without a console) | The user | `shell` |
| `serve` | Login item, service, or command line | No window; proxy and sync |
| `run` | Command line or the server's `agent-runs` | `engine`, headless, one session |
| Declared commands | Command line, other agents, schedulers | `credential-helper`, `sync`, `install`, `doctor`, `login`, … |

A bare invocation with no subcommand and a console is the credential helper, as the bridge behaves today; other agents' configurations depend on it.

## 7. Background process

Third-party agents depend on the proxy and on sync whether or not a window is open. Any composition with `proxy` runs in the background, starts at login, and closing the window does not stop it. Single-instance and hand-off to a running process exist in both codebases; the bridge's implementation is the one kept.

## 8. What does not integrate

The device binary does not link the server runtime, the database layer or `AppContext`. There is no embedded local instance: core is Postgres-only, its kernel floor is about 810 crates, and a device that carried its own instance would lose the property that every call is governed by a real one.
