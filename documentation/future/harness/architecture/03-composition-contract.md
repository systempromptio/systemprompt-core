# 03. The device composition contract

Status: specification draft. The Rust below is shape, not final signatures; S5 fixes them against the `future` contract as it stands then.

## 1. Shared machinery with the server

Decided: the machinery moves down a crate; no cargo feature separates server from device.

| Crate | Holds after the change |
|---|---|
| `systemprompt-provider-contracts` | The composition machinery both sides use: `ExtensionMetadata`, `StandingTag`, `Slot`, `SlotOrder`, `Resolved`, `FromState`, `Factory`, `ProvideFn`, late-bound cells, `ContributionError`, `AbsentDependency`, dependency ordering, withhold-dependants |
| `systemprompt-extension` | The server sink (schemas, migrations, purge, reseal, retention, routes, commands) and a re-export of the machinery, so no server crate changes an import |
| `systemprompt-harness` | The device sink (`HarnessContributions`), device reserved shapes, device admission |

Why not a feature split of `systemprompt-extension`: cargo merges features across a workspace build, so inside core a `server` feature would always be on and a device crate that wrongly used a server type would still compile. With the machinery in a lower crate the compiler enforces the boundary through the crate graph.

One untangling is required: the state type currently names a database handle. That reference moves to the server side.

Acceptance: `systemprompt-harness` and everything it depends on link no SQL parser, database driver or HTTP-server crate. The closure gate proves it.

The engine runs in one of three typed execution modes (`11-execution-modes.md`). A member's tools are written once against the execution environment and run unchanged in all three; a member never asks which mode it is in.

## 2. The member trait

```rust
pub trait HarnessExtension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;
    fn contribute(&self, c: &mut HarnessContributions<'_>) -> Result<(), ContributionError>;
}
```

A member's contributions live in its crate's `src/extension.rs`, as core's members' do. Every sink method only stores.

## 3. The sink

### 3.1 Engine surface (no UI dependency)

| Method | Contributes |
|---|---|
| `tool(T)` | A `Tool`; passes permission rules and the verdict like any other |
| `agent(AgentDecl)` | A primary agent or subagent |
| `command(SlashCommandDecl)` | A slash command |
| `prompt_section(P)` | A system-prompt section at a declared position |
| `instruction_source(S)`, `skill_source(S)` | Where instructions and skills are read from |
| `mcp_server(F)` | An MCP server known at build time |
| `language_server(L)`, `formatter(F)` | LSP and formatter definitions |
| `governor_layer(F)` | A narrowing policy layer (section 6) |
| `session_observer(O)` | A read-only subscriber to session events |
| `projection(P)` | How one manifest entry kind becomes files or state |

### 3.2 Device surface

| Method | Contributes |
|---|---|
| `host_integration(H)` | A managed coding agent (`04-host-integrations.md`) |
| `marketplace_source(M)` | A source of marketplace entries shown to the user |
| `auth_provider(A)` | Reserved to core standing |
| `doctor_check(D)` | A line in `doctor` |
| `machine_step(S)` | An install or uninstall step, with its reverse |

### 3.3 Common

| Method | Contributes |
|---|---|
| `settings::<C>()` | A typed settings block with schema and validation |
| `task(F)` | Owned background work, started and drained by the kernel |
| `provide`, `requires`, `depends_on`, `slot::<S>()` | Typed state and capability-owned slots |
| `cli_command(CommandDecl)` | A subcommand; core's `CommandDecl` and reserved-path check |

### 3.4 Shell surface (declared in the extension; feature `shell`)

| Method | Contributes |
|---|---|
| `brand(BrandDecl)` | Product name, vendor, app id, URL scheme, data root, default instance, icon, displayed version. Exactly one per composition |
| `theme(ThemeDecl)` | A named palette pair (light, dark), syntax palette, type-scale overrides. Contrast checked at declare time |
| `view(ViewDecl)` | A view at a named mount point |
| `part_renderer(R)` | How a transcript part or a tool's row renders |
| `settings_page(P)` | A settings page; generated from the settings schema when no view is given |
| `action(A)` | A command-palette action with optional key binding |
| `strings(Bundle)` | UI text per locale; later members override earlier keys |
| `icons(Set)` | An icon set |

The shell-surface *types* that core must name to admit a composition (`BrandDecl`, `ThemeDecl` as data, mount-point ids) are plain data in core's `device` feature. The toolkit-typed parts (view factories) are defined by the extension's `shell` crate and reach the sink through a slot the shell owns, so core never names a toolkit type.

Mount points (closed list, owned by the shell): sidebar section, titlebar item, composer accessory, dock, settings page, status-bar item, full-window route. The sign-in gate and the governance page are not mount points.

## 4. Composition and phases

```rust
fn main() -> std::process::ExitCode {
    my_product::composition().run()
}

pub fn composition() -> HarnessComposition {
    systemprompt::harness::reference()   // core members named by the enabled features
        .with(Shell)                      // extension members, install standing
        .with(MyBrand)
        .with(MyTheme)
        .with(MyTools)
        .without::<DriveMember>()        // optional core members only
}
```

1. **Declare.** Every `contribute` runs once, before settings or accounts load. Checks: member ids; tool, command, view, theme and host ids for collisions; exactly one brand; no install claims a reserved shape; CLI paths against the reserved list.
2. **Admit.** Reads local settings and the applied manifest. Drops every contribution of a member the user or the instance disabled; withholds dependants; refuses to start when a required member is withheld (the instance's `harness.members` can make a member required). Withholds a host integration whose host the instance does not publish.
3. **Run.** Builds state in dependency order, builds slots, starts the role.

Admission re-runs when a new manifest is applied. A member that becomes disabled is stopped and its contributions withdrawn; one that becomes required and is not linked raises a visible governance fault.

## 5. Standing and reserved shapes

An extension adds members and can remove optional ones. It cannot replace a core member or receive a conferred privilege.

| Reserved to core standing | Why |
|---|---|
| The instance `Governor` | The governed-by-construction property |
| Account storage, sealing, JWT renewal, `auth_provider` | Credentials never pass through extension code |
| Manifest trust and acceptance | Signature, version order, skew |
| The loopback secret and scoped-token derivation | Root credential for every managed agent |
| The model endpoint and base-URL pinning | A member cannot redirect a model call |
| The policy writer and elevation | Runs as SYSTEM |
| Reserved CLI paths: `credential-helper`, `login`, `logout`, `install`, `uninstall`, `sync` | Other agents and administrators call them |
| The sign-in gate and governance page | A fork can theme the trust surface, not remove it |

Standing is a correctness boundary, not a sandbox. An extension is trusted Rust in the same process. The server remains the enforcement point: every model call and verdict is checked by the instance. Device evidence carries a composition digest (member ids and versions) so an instance can recognise or refuse a build.

## 6. Governor layers

An extension may add a `GovernorLayer`. It runs after the instance governor and may only narrow: `Allow` to `Deny` or `Ask`. It can never allow what the instance denied. A layer that errors or times out denies. The reserved governor's one fail-open exception (the tool verdict budget) is not inherited by layers. Enforced by a property test over the verdict lattice.

## 7. Three ways to extend, by trust

| Mechanism | Trust | Delivered by |
|---|---|---|
| Compiled member | Full; reviewed Rust in the extension | The extension's build |
| Manifest content | Data only; signed by the instance | Live sync: skills, rules, agents, commands, themes as data, managed MCP servers, default model |
| MCP server | Out of process; every call passes the verdict | Manifest or user settings |

No dynamic native plugin loading. Rust has no stable ABI and a loaded library would sit beside sealed credentials.

## 8. Themes as data

`ThemeDecl` is `serde`-readable and arrives three ways: compiled by a member, in a manifest bundle, or from a user file. Precedence: user choice, then instance default, then the brand's default. The instance may pin a theme.

## 9. What stays out

- No member receives raw credentials, the JWT, the loopback secret or the sealed store.
- No member can call a model except through the engine.
- No global mutable state; a member's state lives in the typed state built at run. The bridge's remaining process statics (`brand()`'s `OnceLock`, the `lint-bridge-globals` allowlist) are removed by the conversion.
