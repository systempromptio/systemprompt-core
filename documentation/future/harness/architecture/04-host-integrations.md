# 04. Host integrations: managing other coding agents

## 1. What a host integration is

How a device build manages a coding agent it does not run in-process. It is a member with up to four facets. Each facet is optional, so a new agent can start with one.

| Facet | From today's | Answers |
|---|---|---|
| Probe and profile | `HostApp` (`bin/bridge/src/integration/host_app/mod.rs`) | Is it installed; what should its configuration be; write it; remove it; open it; quit it |
| Sync emitter | `HostSync` (`bin/bridge/src/host_sync/mod.rs`) | Project the applied manifest into this agent's own config tree; clear what was written |
| Credential surface | `proxy/scoped_token.rs`, the proxy's route classes | Which scoped token this agent holds and which proxy routes accept it |
| Runner | `HeadlessHost` (`crates/app/agent-runs/src/host/mod.rs`, server side today) | Build a headless invocation; decode its event stream; classify the outcome |

Behaviour that carries over unchanged from the bridge:

- `ProfileRemoval` is typed: `Removed`, `NothingToRemove`, `ManualStepRequired`, `RetainedForSibling`. A manual step is not a failure and not a removal.
- An unattended repair may not raise an operating-system prompt (`install_profile_unattended` answers `NeedsPrompt`).
- Two packagings of one product share a config tree (`shares_config_with`, `profile_identity`, `HostSync::serves`): Codex app and CLI; OpenCode app and CLI. Claude Code and Claude Desktop do not.
- Uninstall removes only what was written, from sidecar ledgers; foreign entries are reported and left.

## 2. The built-in integrations

One member per product, in core, each its own slice.

| Slice | Hosts | Notes |
|---|---|---|
| `host-claude` | `claude-code`, `claude-desktop`; Cowork plugins and artifacts as facets | Managed settings file; Windows policy hive via the policy writer; `allowedWorkspaceFolders` |
| `host-codex` | `codex`, `codex-cli` | One TOML tree; credential helper |
| `host-opencode` | `opencode`, `opencode-cli` | 2.x reads only `~/.config/opencode`; token file written 0600 and read back |
| `host-hermes` | `hermes` | YAML `config.yaml` and `.env` |

The native engine is host `harness`. It needs no integration: it reads the manifest itself.

## 3. Host identity

### 3.1 Built-in hosts stay closed

`HostKind` keeps its closed set and gains one variant and one property.

- `HostKind::Harness`, wire id `harness`, `HostProduct::Harness`, kind `DesktopApp`. Both gateway protocols by default.
- `HostKind::config_owner() -> ConfigOwner { Bridge, Host }`. `harness` is `Host`. The profile-writing and loopback-credential paths filter on it; there is no special case by name.
- Every exhaustive match follows: `models/bridge/{host,instance,manifest}`, `manifest/services/selection.rs`, the gateway bridge routes (`manifest`, `profile`, `data`), marketplace consumer authorization, the well-known bridge document, `agent-runs` `host_for`.
- Forks of the harness are host `harness` with their own brand in `x-systemprompt-client`. Host identity drives manifest targeting and protocol policy; brand drives analytics.

### 3.2 Extension hosts are declared

For coding agents core does not know.

- The instance declares the host in its services configuration (`external_agents` already exists): id, packaging kind, protocols, config owner.
- The server refuses an undeclared host id. The device withholds an integration whose host the instance does not publish. Both sides fail closed; an empty or stale declaration withholds, it never falls through to a permissive default.
- The typed form is a `HostId` newtype in `systemprompt-identifiers` validated as kebab-case, resolved to either a built-in `HostKind` or a declared descriptor. No raw strings on the authorization path.
- This is a change on an authorization path and takes an independent security review.

## 4. Writing an integration for another agent

In the extension (or a published crate a fork depends on):

1. A crate with `src/extension.rs` implementing `HarnessExtension`.
2. `c.host_integration(MyAgent)` with whichever facets exist.
3. `c.depends_on::<ProxyMember>()` if the agent makes model calls.
4. The instance declares the host (3.2).
5. The fork contract suite checks: the written profile points only at the loopback proxy; the credential it holds is a scoped token, never the loopback secret; removal restores the file byte for byte where the integration created it.

## 5. From managing agents to orchestrating them

The runner facet is what turns configuration into orchestration.

| Step | What the device does | Needs |
|---|---|---|
| 1. Bridge behaviour | Configure, sync and proxy each agent | Probe, sync, credential |
| 2. Fleet view | Show every agent on the machine, its health, sessions and verdicts in one place | The bridge's `agent_fleet`, `agent_health`, `agent_verdict`, `feedback_capture` |
| 3. Orchestration | Route a task to the native engine or to an external agent, with one transcript model, one permission surface, one telemetry stream | Runner |

The native engine is one runner among several.

The server's `agent-runs` adapters and the device's runner facets are the same knowledge. They become one set of crates (`runner`), with no server dependency, linked by both. `agent-runs` gains a fifth adapter for the native engine: the server spawns `<binary> run`, reads its event stream and classifies the outcome. For that adapter hooks, resume, structured output and MCP are native.

Today every `HostApp::start_session` returns `None`: no supported host exposes a launch-with-prompt deep link. Orchestration therefore drives CLIs through the runner, not desktop apps. Desktop packagings stay configure-only until their vendors offer an entry point.
