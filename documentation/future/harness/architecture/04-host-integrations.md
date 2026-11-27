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

"Harness" has two meanings in core. In this directory it is the native engine and its host id. In existing code it also names the *evaluator harnesses*: the coding agents whose sessions the feedback pipeline scores (`origin/harness.rs`, `OriginParseError::NotAHarness`, `EvaluatorClient`, `HarnessTargets`). Host `harness` is one evaluator harness among six; the older vocabulary is not renamed. `lint-host-ids` carries the allowlist entries the overlap needs.

### 3.1 Built-in hosts stay closed

`HostKind` keeps its closed set and gains one variant and one property.

- `HostKind::Harness`, wire id `harness`, `HostProduct::Harness`, kind `DesktopApp`.
- `HostKind::config_owner() -> ConfigOwner { Bridge, Host }`, an exhaustive match with no wildcard arm. `harness` is `Host`; the other seven built-in hosts are `Bridge`. Every rule in this section is written on the owner, never on the name, so a later host-owned host inherits all of them and a new variant cannot compile without choosing.
- Every exhaustive match follows: `models/bridge/{host,manifest}`, `models/origin`, `models/feedback`, `models/mcp/execution_source`, `manifest/services/selection.rs`, the gateway bridge routes (`manifest`, `profile`, `data`), marketplace consumer authorization and its installation plan, the well-known bridge document, `agent-runs` `host_for`, and the bridge. `models/bridge/instance` holds typed host lists and no match.
- Forks of the harness are host `harness`. Host identity drives manifest targeting and protocol policy; a fork's brand drives analytics and travels separately (3.1.6).

#### 3.1.1 The vocabularies that carry it

| Vocabulary | Value for the harness | Notes |
|---|---|---|
| `HostKind`, `HostProduct` | `Harness`; wire id `harness` | No config sibling: one packaging |
| `ClientKind` | `Harness`, wire value `harness`, label "Harness"; a member of `ALL` and of `DECLARABLE` | `From<HostKind>` maps the host to it. It is the closed client kind, not a brand |
| User-Agent product token | `harness`, matched exactly against the lower-cased first product token | Evidence tier `user-agent` |
| `EvaluatorClient` | `Harness` | The feedback pipeline can score its sessions. The bridge captures nothing for it: no native session, no capture roots |
| `ExecutionSource` | `HookHarness`, stored as `hook_harness` | Its own value, never another host's: `from_hook_host` names a source for every host and returns `HookHarness` for `harness`. Both stored `source` CHECKs admit it (mcp migration 022 and the base schema), and the seven tool usage report queries that leave a host's built-in tools out by naming the sources name it too. A test holds the enum, both CHECKs, the migration and those queries equal. Core holds no route that records a hook; the member that ingests a harness `PostToolUse` passes `from_hook_host(HostKind::Harness)` |
| Skill rendering form | The Claude form (`SKILL.md`) | The marketplace installation plan renders a harness plan as it renders a Claude one. The match names every client; there is no wildcard arm |
| `agent-runs` `host_for` | `Unsupported` until S9 | The `agent_runs.host` CHECK excludes `harness`; S9 adds the adapter and the migration together |
| Stored CHECK sets | `ai_requests.client_kind` and the client-evidence table admit `harness` (ai migration 059); the per-user host preferences admit it (oauth migration 030); `mcp_tool_executions.source` and `mcp_artifacts.source` admit `hook_harness` (mcp migration 022) | `attested_host` admits it only because that CHECK is held equal to `ClientKind::ALL`; no row is written with it (3.1.4) |

#### 3.1.2 A host-owned host is opt-in per instance

Decided (D42).

- A host whose owner is `Host` is served only when the instance declares it in `external_agents` **and** the entry has `enabled: true`. No entry means withheld. A bridge-owned host keeps the existing rule: no entry means enabled.
- On an instance that serves it, a host-owned host reaches a user only through that user's own enabled preference row. `default_enabled` does not apply to it. The row is independent of the user's bridge-owned hosts: writing it never switches one of them on or off.
- The entry must be complete: `id`, `display_name`, `kind` and `enabled` are required and unknown keys are refused.
- Withheld means absent from the instance's hosts, from every user's `enabled_hosts` and `host_extensions`, refused by marketplace consumer authorization for `host=harness`, and refused by both preference routes, for every write that names it (enable, disable, store a filter, clear a filter).
- **A manifest that names one carries that host's bridge floor.** A bridge released before the variant parses `enabled_hosts` strictly and would fail the whole manifest; the floor (`manifest_min_bridge_version`, the mechanism external marketplace references already use) turns that into the upgrade-required refusal. A manifest that names a host-owned host, in `enabled_hosts` or as a key of `host_model_protocols` or `host_extensions`, carries at least that host's floor. The floor is a property of the host (`HostKind::min_bridge_version`, an exhaustive match), so a later host cannot compile without naming the first bridge that parses it. The floor reaches the users whose manifests name the host, not every user of the instance.
- **The well-known bridge document never lists a host-owned host.** It is unauthenticated, carries no version floor, and its only reader is the bridge, which does not manage such a host.
- A user's last-host guard counts bridge-owned hosts only, so switching off every host the bridge shows cannot leave a user with only a host the bridge never lists.
- A bridge that finds a host id it does not know in its own stored last-sync record drops it with a warning, so a rolled-back bridge still installs and uninstalls. The bridge records only bridge-owned hosts in its last-sync record, so the record is also readable by the bridge released before the host existed.
- An `external_agents` key that names no host is refused at validation.

#### 3.1.3 How far `config_owner = Host` reaches

The bridge writes nothing for, issues nothing to and lists nothing about a host-owned host.

| Bridge path | For a host-owned host |
|---|---|
| Host resolution (enrol, install, the window's handlers) | `Unmanaged`, decided from the owner before any registration is consulted |
| Host-app registry | A registration whose id or profile identity is host-owned is dropped with a warning; a white-label crate cannot make one enrollable |
| Sync-emitter registry and the apply loop | A host-owned emitter is dropped; a host-owned id in `enabled_hosts` enables no emitter; nothing is written and nothing is cleared |
| Sync-only agent list, the Agents card, first run | Not listed, not provisioned |
| Profile inputs | A typed refusal (`HostOwnsConfig`) |
| Scoped-token issue (`credential-helper`, the loopback bearer) | Refused before the secret is read; `credential-helper --host harness` exits 64 |
| Scoped-token acceptance at the proxy | No token authenticates as a host-owned host on any route class |
| Enable toggle | Answers `unmanaged` and sends nothing to the gateway |
| Feedback capture | Nothing captured |

On the server the owner decides opt-in (3.1.2), the well-known document, the last-host guard and who may declare `host_extensions` (3.3). It does not change manifest targeting: a skill may target `harness` like any other host.

#### 3.1.4 Credential, principal and attestation

- The harness holds no loopback secret and no scoped host token, and the stock bridge cannot mint or accept one for it (3.1.3).
- The gateway refuses `host-token` attestation whose declared client is a host-owned host, with a typed classification rejection, whatever principal presents it, and `host-token` attestation for a client kind that has no host.
- Nothing new is granted on a self-declared client kind. `ClientKind` feeds attribution and one behaviour switch (recall, off unless an operator lists the kind).
- The harness signs in through the device sign-in routes the bridge uses (device link, or a PAT exchanged for a session) and fetches its manifest and sends inference with the session that returns.
- **Open (O6): which principal that session is, and so which attestation tier harness traffic records.** If it is a bridge principal, the `bridge-secret` channel is open to it; if it is the user's, traffic is `declared` or `user-agent`. Nothing in the specification or the S0 notes decides it. Needed by S2: S2-A1 issues the test tokens and the golden scenarios assert the tier. Until then no lane may rely on a tier above `declared`.

#### 3.1.5 Both gateway protocols by default

The rule is the absence of a filter. The server holds no per-host protocol default: `host_model_protocols` in the manifest carries only a user's stored overrides. A harness that finds no entry for itself offers the models of both protocol families, and that default is applied in the app, which is the only place a harness can observe it. The per-host model-filter route checks that the instance enables the host before it stores an override. The bridge's per-host client table is not where this rule lives: the harness has no host app, so nothing it runs reads that table.

#### 3.1.6 A fork's brand

Decided (D43). `x-systemprompt-client` stays the closed client kind: a value outside `ClientKind::DECLARABLE` is refused with `400`, and `ai_requests.client_kind` is CHECK-constrained to the same set. The reference harness and every fork send `harness`. A fork's brand travels in its own header and is stored in its own column; both are introduced with `BrandDecl` in S5 (S5-A1 fixes the header name and the validated type). Until S5 a fork is indistinguishable from the reference harness in analytics.

### 3.2 Extension hosts are declared

For coding agents core does not know.

- The instance declares the host in its services configuration (`external_agents` already exists): id, packaging kind, protocols, config owner.
- The server refuses an undeclared host id. The device withholds an integration whose host the instance does not publish. Both sides fail closed; an empty or stale declaration withholds, it never falls through to a permissive default. The built-in host-owned host follows the same rule (3.1.2).
- The typed form is a `HostId` newtype in `systemprompt-identifiers` validated as kebab-case, resolved to either a built-in `HostKind` or a declared descriptor. No raw strings on the authorization path.
- Manifest fields keyed by host (`enabled_hosts`, `host_model_protocols`, `host_extensions`) are typed by `HostKind` today, so a reader fails the whole parse on an id it does not know. A declared host cannot appear in them until they are keyed by `HostId`, with the same version floor (3.1.2) for readers that predate it.
- This is a change on an authorization path and takes an independent security review.

### 3.3 `host_extensions`

A field of the signed manifest: host id to a list of extension ids. Its source is `extensions` on the host's `external_agents` entry; a user's manifest carries the entry only for a host that user has enabled.

Decided (D44):

- **Additive capability only.** An extension id switches on an optional capability the host was built with. It never restricts, enforces or removes anything. That is what makes absence safe: a reader that predates the field, a manifest that omits the host and a user who has the host disabled all end with the capability off. Anything that must hold when the field is missing (a restriction, a required policy) is not an extension; it goes through governance or through a required member (below).
- **Absent means off.** No entry, an empty list, or an id the host does not recognise enables nothing. An unrecognised id is ignored, not an error.
- **Ids are typed and kebab-case.** Lower-case alphanumeric segments joined by single hyphens, starting with a letter, at most 64 characters; `a-` and `a--b` are refused. Validated when services configuration loads and again when a manifest is read. No raw string reaches an enablement decision.
- **Declared only by a host-owned host.** An `extensions` list on a bridge-owned host's entry is refused at validation: the bridge has nothing to switch on. Validation also refuses two entries that name one host through alias spellings, and a key that differs from its entry's `id`, so the signed list cannot depend on map order.
- **Part of the signed manifest.** Canonicalised and signed with the rest of the payload and covered by the entity tag. Omitted from the wire when empty, so the manifest of an instance that declares none is byte for byte what it was.

Relation to `harness.members`:

| | `host_extensions` | `harness.members` |
|---|---|---|
| Says | Which optional capabilities of a host-owned host the instance switches on | Which composition members the instance disables or requires |
| Direction | Additive only | Narrows; can require; never widens |
| Exists | Now, as a manifest field with no server-side policy behind it but the declaration | Contract in S5 (S5-A1); published by the server in S9 (S9-A1) |
| Read by | The lifted harness's enablement, from S1: without it every non-required extension is withheld against a real instance | Device admission, from S5 |

From S5 an extension id is the id of an optional member, and admission treats a listed id as "enabled by the instance" for that member. Two live sources of member enablement are not kept: S5-A1 states in the contract whether `harness.members` absorbs the list or the field stays as its additive half, and S9-A1 publishes accordingly.

The field is on the programme branch and on `future`. It is not on `next`: D13 puts only the host variant there.

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

The server's `agent-runs` adapters and the device's runner facets are the same knowledge. They become one set of crates (`runner`), with no server dependency, linked by both. What that requires of the server's crate graph is in `02-target-architecture.md` section 3. `agent-runs` gains a fifth adapter for the native engine: the server spawns `<binary> run`, reads its event stream and classifies the outcome. For that adapter hooks, resume, structured output and MCP are native.

Today every `HostApp::start_session` returns `None`: no supported host exposes a launch-with-prompt deep link. Orchestration therefore drives CLIs through the runner, not desktop apps. Desktop packagings stay configure-only until their vendors offer an entry point.
