# Changelog

## [0.34.0] - 2026-09-01

### Added

- The `comms-drain` hooks (`UserPromptSubmit`, `Stop`) are installed only when the governance-owning plugin sets `hooks.comms: true` in the manifest. They rode along with every governance owner before.

- OpenCode is a supported host. The bridge writes a `provider.systemprompt` block (the OpenAI-compatible wire, the loopback `baseURL`, the negotiated model list and the `x-inference-protocol` header) and the default `model` into OpenCode's admin-managed configuration — `/etc/opencode/opencode.json`, `/Library/Application Support/opencode/opencode.json` or `%ProgramData%\opencode\opencode.json` — which OpenCode layers above every user and project file, so no local config can route inference around the gateway. The write is direct where the process may, escalates through the existing `sudo`/`osascript` path on macOS and the UAC child on Windows only when refused, and is skipped entirely when the file already says what it would say. The API key goes to the user's `auth.json` (0600); MCP connectors go to the user's global `opencode.json` and skills to `~/.config/opencode/skills`, both user-owned because unattended sync can never prompt. Skill folders are kebab-cased and the front matter `name` is forced to match, since OpenCode rejects a skill whose name differs from its folder; two ids that collapse to one folder are refused before anything is written. The probe reads the managed file and the `ai.opencode.managed` MDM domain, never user scope, and finds the `opencode` binary in the usual install prefixes even when the GUI's PATH lacks them.
- `HostApp::can_open` lets a terminal-only host say so, and the verdict then offers no Open button — Codex on Linux and every CLI host used to get one whose only outcome was an error toast.
- The Hermes card has a logo; it rendered an empty glyph. Hermes also gained the unit coverage it shipped without: probe, install/merge/remove, `.env` handling and the sync emitter.

### Changed

- Managed skills for hosts that read `SKILL.md` folders directly (Hermes, OpenCode) go through one writer, `integration::managed_skills`, with the sidecar, pruning and front-matter rendering in one place; the Codex marketplace writer shares its renderer. Hermes's `config.yaml` MCP write is atomic, as its profile write already was.

### Fixed

- The Hermes host profile never routed anything. Verified against Hermes Agent 0.21.0: `model.base_url` is only consulted after `model.provider` selects a provider, and the profile left `provider` at its default `auto`, so Hermes answered "No LLM provider configured". The profile is now a named `providers:` entry selected by `model.provider`, which is how Hermes reaches any non-built-in endpoint.
- `model.api_mode` was written as `openai`, which is not a value Hermes knows — its vocabulary is `chat_completions`, `codex_responses`, `anthropic_messages` and `bedrock_converse`. The key was silently discarded. The wire format the gateway serves is now named explicitly as `chat_completions`.
- The loopback secret written to `HERMES_HOME/.env` was never read. Hermes host-gates its bare `OPENAI_API_KEY` fallback to openai.com and openai.azure.com, so a `127.0.0.1` endpoint resolved no credential and Hermes sent its `no-key-required` placeholder — which the proxy correctly refused with "bad loopback secret". The entry now carries `key_env`, so the secret stays in `.env` at 0600 and is still found.
- The model was written to `model.model` while Hermes' own installed config.yaml always ships a `model.default`, and `default` wins when both are present — so the negotiated model was inert. The profile now writes `model.default`.
- Uninstall removes only this bridge's `providers:` entry, leaving a user's other named providers in place.

## [0.33.0] - 2026-08-31

### Removed

- The "Governed requests" table on the Activity page, with its in-memory request ring, the `requests.recent` IPC command, the `request` event channel and the 30-second poll of `GET /v1/bridge/decisions`. The table attributed every MCP request to an agent named `unknown` — the label came from the `User-Agent` header, which MCP clients do not send — and its Verdict and Tokens columns could never be filled for MCP traffic. Governance is recorded by the gateway, not asserted by the proxy.
- The `x-systemprompt-bridge: 1` upstream header, which nothing consumed. The gateway identifies bridge traffic by the `client_id` claim its tokens now carry.

### Added

- Hermes Agent Desktop is a supported host. The bridge merges `model.base_url`, `model.api_mode` and `model.model` into `HERMES_HOME/config.yaml`, writes the API key to `HERMES_HOME/.env`, publishes managed skills into the Hermes skills directory and prunes only the ids it wrote, and probes the running app. Hermes reads the same plain config on every OS, so unlike the Codex host there is no macOS configuration-profile path.
- The Marketplace listing groups skills and artifacts under the plugin that ships them, with a sticky header naming the plugin and its item count. An item two plugins ship appears under each, since deduplicating it to one header would misreport who ships it; anything with no owner — a plugin, an MCP server, an item from an external source — falls under "Ungrouped", and a listing where nothing has an owner renders flat rather than under a single redundant header.
- The Cowork artifact sinks are replaced with exactly what the manifest carries, so an id the manifest has stopped naming is dropped instead of accumulating. The version stamp hashes only the ids the manifest carries, so it matched even while a sink still held records the manifest had dropped, and such an install took the "up to date, skipping" path forever. A non-empty artifact set is authoritative; an empty one still preserves the store and warns.

- A real light theme, and an Appearance control in Settings. Colour scheme and contrast were wired as one axis, so `prefers-color-scheme: light` handed the user the *elevated-contrast dark* palette — a darker UI — and no light theme existed. They are now `data-theme` (dark/light) and `data-contrast` (default/elevated), composing in all four combinations, each with a stored override above the OS preference so a machine with neither setting is no longer without recourse.
- `scripts/lint-bridge-i18n.sh`, run by `just check` and the Quality workflow. It fails on a message id referenced from JS or Rust but absent from `bridge.ftl`, on a catalogue key nothing references, on a literal `t("id")` written without an English fallback, and on an interpolated `data-l10n-id` it cannot resolve.
- A keyboard-shortcuts list under Help & Support, with platform-correct modifiers. There was no shortcut list anywhere in the app.

- The window title bar follows the app theme on Windows. `DWMWA_USE_IMMERSIVE_DARK_MODE` is set from `Window::theme()` at creation and re-asserted from the `ThemeChanged` event, so a dark app no longer wears a white system title bar — and a user who asked their OS for light mode does not get the mismatch back with the colours swapped.
- Start at login, as a Task Scheduler logon task on Windows and a LaunchAgent on macOS, toggled from Settings and from the tray. The task explicitly clears `DisallowStartIfOnBatteries`, whose Task Scheduler default would silently skip the logon on a laptop — the whole point is that the governing proxy is up before any agent runs.
- Settings gained a startup-and-updates row over a new `settings.get` / `settings.set` IPC pair, which is also where `update.automatic` is finally reachable from the UI.
- Windows toast notifications, raised on five transitions the app already computed and discarded: a sync that finished with host failures, an update ready to install, the gateway becoming unreachable, MCP auth breaking, and a session about to expire. Gateway, MCP-auth and session signals are edge-triggered, so a condition that persists is announced once rather than every probe. Update-*check* failure stays silent, as before.
- A governed-request stream. The proxy now keeps a 500-entry ring of what it forwarded and what it refused — time, agent, method, path, status, latency, tokens and verdict — served over a new `requests.recent` IPC command and a `request` emit channel, and rendered by `<sp-request-stream>` with search, a denied-only filter and copy. The two loopback rejections (non-loopback `Host`, bad or missing loopback secret) are recorded as denials rather than only logged as prose, so a refused request appears as a refusal instead of a gap.
- `<sp-governance-strip>`, a live one-line answer to "is my traffic being governed". It consumes the four `proxy_stats` fields that were computed every second and read by nothing — `forwarded_total`, `last_status`, `last_latency_ms`, `last_forwarded_at_unix` — and degrades to "No traffic in the last 2 hours" and to "Proxy not responding — agents are not being governed".
- `<sp-setup-health>`, the `ValidationReport` rendered as levelled rows, failures first. The report has been computed, structured and levelled all along; `rendered()` text into the activity log was the only way to see it, truncated at a fixed row height with no wrap.
- The state snapshot carries `last_validation`, `last_validation_at_unix`, `last_sync_report`, `provider_health` and `malformed_plugin_count`. All five were computed and discarded at the IPC boundary. `last_sync_report` is the structured `SyncSummary` beside the rendered string the tray uses, so `host_failures` and `diagnostics` become actionable rows with a host id and an error, and `malformed` names the bundles that failed to parse.
- `GET /v1/bridge/decisions` on the gateway, and a 30-second poller that joins its verdicts onto the request ring. The gateway already writes a governance decision on allow as well as deny, keyed on the AI request id it returns as `x-systemprompt-request-id`; the bridge captures that header per request, so the stream's verdict column is the platform's real decision rather than an assertion that governance happened.
- `activity.recent`, backfilling the activity log from the Rust ring on connect. The ring has held 1000 entries all along and nothing ever asked for them, so every webview reload started at "Ready." and forgot everything else.
- The usage tap records `cache_read_input_tokens`, `cache_creation_input_tokens` and the response `model`. Groundwork only — no cost figure is shown, because one computed without cache tokens would be wrong on every Claude model.

- A first-close notice explaining that closing hides the app to the notification area rather than quitting it.
- A startup check for the WebView2 Evergreen runtime. Without it `build_as_child` failed inside a `windows_subsystem = "windows"` process with no console and no window, and the app simply never appeared; it now says what is missing and opens the bootstrapper. Any other webview failure now raises a dialog too.
- The settings window remembers its position, size and maximised state. A rectangle that no longer intersects any attached display is discarded rather than restored off-screen.
- Tray: a "Check for updates" item, a "Start at login" checkbox, and a tooltip carrying the live identity and last-sync text instead of a static brand string.

### Changed

- The activity log reads the severity Rust sends instead of guessing it from the message text with `/(fail|error|refused|denied|reject)/i`, and stamps each line with the entry's own `ts_unix` rather than the time it happened to arrive — any queuing, batching or replay mislabelled it. It also gained search, a level filter, copy, and click-to-expand, because a fixed 18px row cannot wrap and the one place errors surfaced was the one place they could not be read.
- The virtualised log viewport is `aria-live="off"`. It rewrites itself on every scroll frame, so as a live region it re-announced its whole visible window as a screen-reader user merely scrolled, and a 1000-entry backfill would have done the same a thousand times at once. New lines go to the app's single polite announcer instead; `role="log"` stays on the container.
- `validate` appends a one-line result to the activity log and sends its structured `lines` over IPC, instead of dumping the whole multi-line rendered report in as a single truncated entry.

- On Windows the `muda` menu bar is no longer attached to the HWND; `menu.rs` is macOS-only. It rendered as a system-coloured Win32 strip between the title bar and the dark UI — a third chrome band. Its commands moved to a new overflow menu in the web topbar.
- `notify_user` split into a blocking `alert_user` and a non-blocking `notify_user`. The Windows `alert_user` is a native `MessageBoxW` rather than a spawned PowerShell `MessageBox.Show`, which removes a visible console window, several hundred milliseconds of latency, and a command-line injection sink that stripped quotes instead of escaping them.
- The tray icon on Windows is the 16x16 frame from the app `.ico` rather than the 1024px window icon resampled down. The alert dot is inset so it is no longer half-clipped at the bitmap edge.
- The window's minimum size is logical, not physical. It was `PhysicalSize`, so at 150% scaling the effective minimum fell to 533x400 logical and at 200% to 400x300 — far below anything the layout is built for.
- Release builds no longer ship a right-click-inspectable webview: `with_devtools` follows `debug_assertions`.
- Tray menu labels go through the i18n catalog like the menu bar's already did.
- A gap of more than a minute between event-loop passes is read as a resume from sleep and forces an immediate gateway and host re-probe, instead of leaving a stale alert dot in the tray for a full probe interval.
- Tray events are drained once per loop pass, in `new_events`. The second drain in `about_to_wait` raced it, so whether a menu click dispatched immediately or a loop iteration later was arbitrary.
- The inert `-webkit-app-region` rules are gone from the topbar CSS. There is no `WM_NCHITTEST` handling and no drag IPC, so the drag region never worked in a wry-hosted child webview; it only made the window look like an abandoned frameless attempt.
- `Brand` gained `aumid`, `autostart_label`, `autostart_task_name` and `assets.app_icon_ico`. White-label builds that construct a `Brand` literal must supply them.

- Cowork egress is unrestricted by default. `install --apply` no longer writes `coworkEgressAllowedHosts` into the Windows policy key or either macOS payload; the key is omitted so Cowork's own default applies. Pinning it to loopback (added in 0.29.0) meant a stock install had no internet access at all — every web fetch failed as an organization egress block. The lockdown is now an explicit opt-in via `install --apply --egress-allowed-hosts loopback` or `<PREFIX>_EGRESS_ALLOWED_HOSTS`, which also accepts a comma-separated host list. The printed MDM snippets carry it commented out.

### Fixed

- `t()` returned the message *id* for a missing key. An id is a truthy string, so all 117 `t("id") || "English"` fallbacks in the tree were unreachable code and a missing key rendered its own id at the user — the Status pane shipped printing `status-cloud-reach-label` where "Reachability" belongs. `t()` now returns `undefined`, the fallbacks work as written, and 10 absent keys were added: the four Status and Marketplace ids, and all six `marketplace-cat-*`, which were reached through an interpolated id. Nine `tray-*` ids the tray menu referenced did not exist either, so the system tray was printing raw ids in the same way.
- The Marketplace could not be operated by keyboard at all. Items were roleless `<li>`s with `aria-selected` and no `tabindex` inside a plain `<ul>`, and the category rail put `role="tab"` and `tabindex="0"` on every `<li>` with no key handler — five tab stops that did nothing, because `SpElement` bound only `click` and `input`. The list is now a listbox with a roving tabindex, the categories are real buttons in a tablist with `aria-controls`, and `data-action` responds to Enter and Space on non-button elements.
- The agent drawer declared `aria-modal="true"` with no focus trap and nothing `inert`, announcing that the rest of the page was unavailable while leaving it fully reachable behind the scrim. Tab now cycles inside the panel and the background is inert while it is open.
- Focus was invisible on most of the app. `button.css` had no `:focus-visible` for the base selector, so every ghost and secondary button showed nothing, and the checkbox rule replaced the outline with a ring built from `--sp-accent-soft` — 1.17:1 measured, against the 3:1 WCAG 1.4.11 requires. There is now one `--sp-focus` set, solid and measured at 6.30:1 on the most elevated surface, and all 30 keyboard stops carry a visible indicator.
- Two design tokens were undefined and painting their fallback instead: `var(--sp-focus, currentColor)` gave the status pills a ring in the wrong colour and `var(--sp-danger, var(--sp-muted))` rendered the profile menu's errors in muted grey. `lint-bridge-css-tokens.sh` now fails on an undefined token in fallback position, which is the hole both had been hiding in.
- Error toasts auto-dismissed after 8 seconds, a WCAG 2.2.1 failure for a message the user has to read, and the toast declared `role="status"` and `aria-live="assertive"` at once while toggling `hidden` on the live region itself. Errors now persist until dismissed, the role follows severity, and the region stays mounted.
- The sync pill and the setup agent list were `aria-live="polite"` on containers that repaint wholesale every probe tick, so they chattered. Both now push to one de-duplicating announcer, which is also what the four Status section badges use — they previously flipped from `checking…` to a real state with no announcement at all.
- Copy and terminology. *Host app* is *agent* everywhere; the MDM artefact is always a *configuration profile*; *Re-check* is the single verb for what was Re-check, Verify, Re-verify, Validate, Run validate and probe; *personal access token* is never *PAT*; *sign out* is never *log out*; and session expiry reads "expires in 8 minutes" rather than a JWT and a raw second count. The Account identity and plan tables no longer label rows with database column names (`user_id`, `tenant_id`, `jwt issuer`), and the model-filter checkboxes no longer label themselves with wire-protocol names alone.
- The Profile pane, the whole gateway sign-in form, the Marketplace empty states and change badges, and every hardcoded `aria-label` now go through the catalogue. `data-l10n-aria` was implemented and used zero times.
- `text-transform: uppercase` is gone from the 21 label rules that used it. It made the visible name diverge from the accessible name, uppercases German nouns and means nothing in CJK; those labels moved from 9–11px to 11–12px with the wide tracking removed, which also lifts the small muted text the review flagged. An `opacity: 0.6` stacked on `--sp-muted` measured 3.03:1 and is gone.

## [0.31.0] - 2026-08-26

### Added

- Host-targeted skills: skills carrying a `hosts:` list are delivered only to the hosts they name.

### Fixed

- The minted-JWT cache is credential-scoped: entries carry a fingerprint of the PAT that minted them and are discarded when the PAT on disk changes, so two identities against one gateway no longer share a slot.
- `logout` removes `last-sync.json` and `user.json`; the sync replay guard no longer compares the next account's first manifest against the previous account's version.
- A freshly minted token the gateway refuses is cleared from the cache instead of being replayed as valid on the next run.

## [0.30.0] - 2026-08-25

### Added

- The credentials-rejected sync error names the gateway URL, the JWT identity it presented, the config and PAT file paths it read, and whether `XDG_CONFIG_HOME` or the config override variable redirected them — so a CLI and a desktop app resolving different credential directories is visible in the error itself. It also distinguishes a rejected cached token from a rejected freshly minted one.
- A sign-in performed outside the running app (for example `login --code` in a terminal) is picked up without a restart: the proxy re-reads credentials when the PAT or config file changes on disk.
- A user-initiated sync that fails with a credential error opens the settings window for re-authentication instead of ending in a toast.
- The sync summary reports gateway diagnostics carried in the manifest and counts the skills the plugin bundles actually install, with a warning when the manifest lists more skills than any bundle delivers.
- `HostId` is a typed identifier; host ids in GUI events and handlers are no longer raw strings.

### Fixed

- White-label bridges are no longer rejected by the gateway's bridge-version floor. The manifest floor check and the heartbeat report now use the core bridge library's version (`brand::COMPAT_VERSION`) instead of the brand's own display version, whose independent numbering read as ancient against `min_bridge_version`.

### Changed

- A manifest response the bridge cannot parse is reported with the schema and bridge version floors when the payload carries them — an out-of-date bridge is told to update instead of seeing a decode error — and otherwise with a bounded snippet of the response body.

## [0.29.0] - 2026-08-24

### Fixed

- Signing in no longer leaves the previous credential in charge. `login`, `set_gateway_url`, and the interactive sign-in wrote the new credential but never cleared the cached JWT, and every reader consulted that cache first — so a token the gateway had already rejected kept being sent until its TTL lapsed, and neither re-signing in through the app nor `login --code` on the command line changed anything. All three now discard the cached token.
- A cached token is scoped to the gateway that issued it. The cache was documented as keyed by gateway identity but was a single unkeyed file, so repointing the bridge replayed the previous gateway's token at the new one. A token minted elsewhere is now refused and deleted on read.
- A credential the gateway rejects is discarded and re-minted once, and sync continues. A 401 or 403 on the manifest was terminal, which turned one bad token into a permanently wedged install. The error now surfaces only when a freshly minted credential is also refused, and says so.
- Signing in preserves the rest of the configuration. The config file was rewritten from scratch on every login, silently dropping `[sync] pinned_pubkey` — which quietly re-enabled trust-on-first-use — along with `[claude]`, `[cowork]`, `[mtls]`, and `deployment_organization_uuid`. Only the keys a sign-in owns are now replaced.
- `doctor` no longer reports that the gateway accepted credentials on the strength of a cached token it never presented. It also reports whether the cached token was minted for the configured gateway, and whether host launchers point at the running binary and version.
- Nine interface strings resolved to their own identifier, so sign-out, sign-in progress, gateway saving, and validation showed raw slugs such as `logout-success`. A test now fails when a string the code asks for is missing from the catalogue.

### Added

- The bridge refuses to sync against a gateway that requires a newer bridge, naming both versions, and updates itself to meet the floor. The update runs through the existing signature- and digest-verified download path. Set `automatic = false` under `[update]` to disable it; the key is also readable from managed policy.
- An unauthorized sync error offers a Re-authenticate action rather than printing a command line to a desktop user.

## [0.28.0] - 2026-08-23

### Fixed

- The instance host gate no longer fails open before the first sync. `enabled_hosts` comes from the last signed manifest, which does not exist on a fresh install, so `gui/hosts/serde.rs` skipped the filter entirely and the GUI offered every host the build registers — including ones the installation disables. `HostsPayload` now carries `hosts_gated`, so surfaces that offer to *act* on a host can withhold the action until the gate is authoritative. It reads a new `AppStateSnapshot::manifest_synced()`, deliberately **not** `!enabled_hosts.is_empty()`: an instance may disable every host, and that empty list is a real answer from a good manifest rather than a missing one — gating on emptiness would have pinned such an install on "checking" forever.
- The last-sync record is read even when the org-plugins directory does not resolve. It was nested inside that branch in `reload_into`, though nothing it restores — `last_sync_summary`, `enabled_hosts`, `host_model_protocols` — depends on the directory, so a machine without one silently lost the manifest's host gate and model protocols.
- `agents_onboarded` is durable. It lived only in the in-memory snapshot, so finishing setup bought nothing beyond the current process: the next launch re-derived "needs setup" from whether any host still reported an installed profile, and a user who had completed setup was put back through it after uninstalling the last profile or probing it stale. `setup.complete` now writes an `onboarded.json` sentinel beside `first-run.json`, `reload_into` reads it, and `auth::setup::clean` removes it alongside the first-run record.

## [0.27.0] - 2026-08-23

### Changed

- The gateway URL comes from the config file only; the `GATEWAY_URL` environment override is gone.

### Fixed

- Windows: a sync that finds the Cowork org-plugins directory out of scope raises a single elevation prompt to provision it and retries, instead of failing outright. One attempt per process, so a declined prompt is not re-fired by the GUI auto-sync, tray retries, or `sync --watch`.
- A browser launched by the bridge no longer inherits the bridge's stdio.

## [0.26.0] - 2026-08-19

### Added

- MCP protocol 2026-07-28 with dual-lifecycle compatibility (rmcp 3.1.3). The bridge's proxied MCP path serves 2026-07-28 clients statelessly with per-request `_meta` negotiation and `server/discover`, while legacy clients keep the initialize handshake and sessions. The internal MCP proxy streams response bodies with preserved headers, follows `tools/list` pagination cursors, and forwards SEP-2243 operation headers.
- Per-instance host gating: a host whose `external_agents` catalog entry sets `enabled: false` is omitted from the signed manifest's enabled hosts and cannot be enabled per-user (the gateway answers 422).

### Changed

- Windows: `bridge install --apply` provisions the Claude policy keys and the org-plugins directory ACL in one elevated job, replacing the sync-time `icacls` grant.

## [0.25.0] - 2026-08-18

### Breaking

- **Breaking:** the gateway signs manifests as a `SignedManifestEnvelope { payload, signature }`, where `payload` is the manifest's JCS-canonical JSON signed byte-for-byte. The bridge verifies over those exact bytes before deserialising, so fields added by newer gateways no longer break older bridges. `min_schema_version` declares the oldest schema that can safely consume a manifest and is refused with an explicit upgrade message, replacing the opaque signature error on version skew. Both `CanonicalView` copies are deleted.

### Fixed

- Windows: a sync targeting the Cowork host from a process that cannot write the system org-plugins path now fails with elevation guidance instead of silently writing to a per-user directory Cowork never scans; `doctor` reports the same condition.

## [0.24.0] - 2026-08-07

### Added

- The signed manifest carries `allow_claude_ai_connectors` (from the instance's `bridge_policy:` services config). When set, the Claude Code managed-MCP policy writes `allowAllClaudeAiMcps: true` alongside the managed server allowlist, so claude.ai first-party connectors keep working under `managed-mcp.json`; when withdrawn, the key is removed rather than left stale, and `clear_policy` removes it too.
- Self-update from the gateway. `update` (with `--check` and `--yes`) checks `/v1/bridge/latest`, streams the artifact while hashing, verifies the SHA-256 before anything executes, then swaps per platform: macOS unpacks the zipped `.app` with `ditto` and re-verifies `codesign` + `spctl` before replacing the bundle; Windows renames the running exe aside and sweeps the leftover at next start; Linux writes beside the target and renames over it. Every path rolls back on failure. The GUI rail profile button becomes "Click here to update" when a build is available and "Restart to finish updating" once installed; `--check` exits non-zero when a build is available, for cron probes. Version comparison is semver, not string ordering.
- `Brand` carries the downstream crate's version, and the footer, rail, `--version`, diagnostics, and heartbeat `bridge_version` all read it. `env!("CARGO_PKG_VERSION")` expanded inside this library, so every white-label build reported core's version instead of its own — the updater would have compared the wrong numbers.
- The Codex provider profile pins approval and sandbox policy: `approval_policy = "never"` with `sandbox_mode = "workspace-write"` (network access enabled), so an unattended managed Codex neither stops to ask nor gains full-disk access.

### Fixed

- A `/mcp/<name>` registry miss re-reads `mcp-servers.json` before answering 404. On a fresh install the proxy starts before the first sync writes the fragment, and the sync publishes into its own process memory — so every managed MCP request 404'd for the life of the proxy and the session came up with zero tools.
- The loopback proxy injects an SSE comment frame every 15 s into proxied `text/event-stream` responses. An MCP session rides a long-lived stream; when its socket died silently, the host app queued tool calls against it until a TCP timeout fired minutes later — observed as ~147 s stalls in Cowork against a healthy upstream. Keepalives make a dead connection fail fast so the host reconnects in seconds.
- The Codex host declares `ApiSurface::OpenAi`. `accepted_surfaces` sat at the trait's empty default, so no model protocol was negotiated, `host_model_view` offered no compatible models, and the `x-inference-protocol` header was skipped — the host synced its MCP and plugin half but silently installed no model provider profile.
- macOS: the Codex probe reads the installed profile from managed preferences (`config_toml_base64` in the `com.openai.codex` plist, user scope before device scope) instead of the Linux `/etc/codex/config.toml`, which never exists there — a successful install no longer re-verifies as "profile not installed". `install --apply-mobileconfig` also tells the user the profile must be approved in System Settings, since `open -g` parks the payload there without ever surfacing the approval sheet, and `install_action_label` no longer claims the profile was already loaded.

## [0.23.0] - 2026-08-06

### Changed

- macOS: `install --apply` now acquires administrator privileges when needed, instead of hanging (GUI, no TTY) or silently downgrading to a warning (`managed-mcp.json`). New helper `install/elevate.rs` picks the right prompter based on whether stdout is a TTY: `sudo /bin/sh -c` on a terminal (sudo prompts on stdin, timestamp-cached), or `osascript` `do shell script … with administrator privileges` in a GUI context (native macOS credential dialog). Both the Claude Desktop managed-preferences plist writer (`install/mdm/macos.rs::apply` / `::remove_profile`) and the Claude Code CLI enterprise policy writer (`install/managed_mcp.rs::apply_policy` / `::clear_policy`) route through it. A diff-first check reads the current on-disk file body first, so idempotent sync ticks and re-runs of `install --apply` with an unchanged manifest do NOT prompt. When the user cancels the dialog we log a distinct "declined" state and fall back to the per-plugin `.mcp.json` path rather than a spurious "unenforced" warning. The old "here's the file body, deploy it via MDM yourself" fallback text is removed — MDM stays available for genuine fleet rollouts, but is no longer the every-user fallback.

### Fixed

- macOS: bridge no longer segfaults when it loses focus (opening System Settings via `install --apply-mobileconfig`, clicking away, Cmd-Tab). Root cause was a two-`objc2` split in the dep graph — `winit 0.30` pinned `objc2 0.5` while `wry`/`muda`/`tray-icon` used `objc2 0.6`. On `-[NSWindow resignKeyWindow]`, a notification observer registered by the 0.6-side crates would dereference a weak reference to a window whose bookkeeping lived on the 0.5-side, reading a bogus pointer (`EXC_BAD_ACCESS`). Bumped `winit` to `0.31.0-beta.2`, which drops its direct `objc2` dep and lets the whole stack resolve to one `objc2 0.6`. Migrated the GUI to the new winit API: `ApplicationHandler` is no longer generic, `EventLoopProxy` no longer carries a payload, `resumed` is renamed to `can_create_surfaces`, user events are drained via a new `proxy_wake_up` hook (a small `UiEventProxy` wrapper preserves the existing `send_event(UiEvent)` call sites), `Window` is a trait so `create_window` returns `Box<dyn Window>`, and `WindowAttributes` macOS extensions now go through `with_platform_attributes(Box::new(WindowAttributesMacOS::default()...))`. `cargo tree -e no-dev` now reports a single `objc2 0.6.4`.
- macOS: the earlier `open -g` mitigation is preserved for cleanliness (System Settings still shouldn't steal focus on profile install), but is no longer load-bearing — the crash was fixed at source by the `winit` bump.
- macOS: settings window no longer opens with `with_maximized(true)` (the initial-maximized attribute was tangled up in the earlier misdiagnosis of the resign-key crash; dropping it keeps the same 1100x760/min 800x600 default). Kept because the user can still zoom manually.
- macOS: WebView is now built with `WebViewBuilder::build_as_child(&window)` instead of `.build(&window)`. Root cause of the resign-key `SIGABRT` was that `.build()` on macOS *replaces* the `NSWindow`'s `contentView` with wry's `WryWebViewParent`; winit-appkit 0.31's `windowDidResignKey` handler then calls `self.view()` (`window_delegate.rs:203`) which downcasts the current `contentView` to `WinitView` — the downcast fails with an `Err` containing the `WryWebViewParent`, and the `.unwrap()` inside `view()` (`window_delegate.rs:878`) panics. The panic hook we installed at `obs::install_panic_hook` captured the message to `~/Library/Logs/astound-bridge/bridge-crash-*.log` and named it exactly. `build_as_child` adds the WKWebView as a subview of the winit-created `WinitView`, leaving the `contentView` cast winit-appkit relies on intact. We now also honour `WindowEvent::SurfaceResized` to re-set the webview's bounds so it stays flush with the window.
- macOS: double-clicking the `.app` bundle now opens the GUI instead of silently emitting a JWT and exiting. `should_default_to_gui()` was `const false` off Windows; it now shares the Windows `!isatty(stdout)` heuristic so Finder/launchd invocations (no controlling terminal) route to `cmd_gui`, while Terminal invocations still fall through to `cmd_run`.

## [0.22.0] - 2026-07-30

### Breaking

- **Breaking:** `GatewayClient::provision_oauth_client` takes `&BearerToken` and `GatewayClient::pat_exchange` takes `&PatToken`, in place of `&str`; `plugin_oauth::{ensure_creds, refresh_creds, mint_or_refresh_plugin_token}` likewise take `&BearerToken`. The parameter these threaded was named `pat` while the proxy passed it the bridge JWT, which reads as PAT-only auth and led to the incorrect conclusion that device-cert users could not use hooks at all. Migrate by passing the typed token — `&auth_token.token` rather than `auth_token.token.expose()`. Both newtypes zeroize on drop and redact in `Debug`, so the `&str` hops were also discarding the secret handling the types exist to provide.

### Fixed

- Bridge subcommands report failures on stderr, not only to the rolling log. `TeeWriterImpl` wrote to stderr solely as a *fallback* for a missing file appender, so in the normal case `sync` exited non-zero with nothing on stdout or stderr and the reason reachable only in `~/.local/state/<brand>/bridge.<date>.log`. Every subcommand was affected — they all report through `tracing`, directly or via `obs::output::diag`. WARN and above now reach both sinks; INFO and below stay file-only, so `run`'s per-request proxy chatter does not flood a console or journal.
- `gateway_aligned_endpoint` aligns a loopback `token_endpoint` against the gateway actually being dialled rather than against ambient `config::load()`, which made a pure URL transform depend on process-wide state and could re-point the endpoint at a gateway the client was not talking to.
- `doctor`'s "hook token mint" warning no longer claims provisioning "runs on first sync after login". `ensure_creds` is called only from `mint_or_refresh_plugin_token`, lazily on the first plugin hook request; the old wording sent operators looking at sync for a fault that was not there.
- The `login` command's doc comment described "browser-based device-link authentication" while the implementation only stores a pasted PAT. It now says so, and records that a device certificate is the only credential that renews unattended: the proxy re-authenticates per request, so a device-link-only configuration reopens a browser on every hook and dies with `authentication timed out after 10s`.

### Added

- `login --code <exchange-code>` redeems a one-shot code from `admin bridge issue-code` for a durable PAT, via the same `/v1/auth/bridge/session-pat` endpoint the desktop GUI's sign-in uses. That endpoint was reachable only from the GUI, which is macOS/Windows-only, so on Linux there was no browserless way to bootstrap a credential at all: the session provider always opens a browser, and `login` accepted only an already-issued `sp-live-…` token that nothing headless could produce. `--device-name` labels the PAT (defaulting to the hostname) so an admin can revoke by machine.
- Linux `install --apply` writes the environment a login shell needs instead of erroring. It emits `$XDG_CONFIG_HOME/<brand>/env.sh` exporting `ANTHROPIC_BASE_URL` (the loopback proxy origin) and `ANTHROPIC_AUTH_TOKEN`, plus a marker-delimited managed block in `~/.profile` that sources it. The token is read from the loopback key file when the file is sourced rather than baked in, so a rotated secret needs no rewrite and an absent one leaves the variable unset instead of setting an invalid credential. Re-running install replaces the block rather than appending a second one, and both files are written via temp-file + rename so a crash cannot truncate a user's dotfile. `uninstall` removes the env file and the block, leaving the rest of `~/.profile` byte-identical. Only the pair proven end-to-end is written; the `CLAUDE_INFERENCE_GATEWAY_*` keys the old snippet advertised are not.
- `install --apply-schedule` registers a second systemd user unit on Linux, `<binary-name>-proxy.service`, which runs `<binary> proxy` with `Restart=always`. macOS and Windows run the loopback proxy inside the GUI process; on Linux nothing owned its lifecycle, so it had to be started by hand every session. Rendered from a separate template with its own path — the existing `template`/`split_systemd_unit` contract of one label and one (service, timer) pair is unchanged. `uninstall` disables and removes it.
- `doctor` gains three checks: whether the loopback proxy is listening (reusing `integration::proxy_probe`, which existed but was never wired in), whether the proxy's systemd unit is present and active (Linux only), and whether the `org-provisioned` marketplace is registered with the Claude Code CLI. The last names the silent failure where `sync` skips its marketplace emitter because the CLI is absent, leaving `claude plugin list` empty with every other check green.

### Changed

- **Behaviour change:** `install --apply-schedule` on Linux no longer fails when `systemctl` is missing or `--user` has no bus — a container, or WSL without systemd. It wrote the unit files first and then returned an error, leaving the install half-done for no gain. It now keeps the files, reports a warning naming the commands to run by hand, and succeeds. This applies to the pre-existing sync timer as well as the new proxy service, and follows the macOS path, which has always ignored `launchctl` failure.
- `mtls.cert_keystore_ref` is read on Linux, where it names the path to the device certificate; `~` is expanded. Previously every use in the codebase was `.is_some()`, so the key meant only "mTLS is configured" and the certificate could be named solely by `<PREFIX>_DEVICE_CERT`. The env var still wins where both are set, so existing setups are unaffected. macOS (Keychain label) and Windows (cert-store thumbprint) address certificates differently and ignore the value — this is Linux-only. `keystore::platform_source` therefore takes an `Option<&str>` on every platform.

## [0.21.0] - 2026-07-29

### Changed

- Dependency refresh only; no bridge source changed since 0.20.0. The lockfile moves `rmcp` from `3.0.0-beta.1` to the released `3.0.0`, which pulls `base64` 0.23.0 alongside the existing 0.22.1. `rmcp` is a transitive dependency here — the bridge declares none of it directly — so there is no behavioural change to the bridge's own surface. The release exists so the published binary matches the tree rather than a prerelease dependency.

## [0.20.0] - 2026-07-28

### Fixed

- `BridgeError.detail` and `IpcReplyPayload.value`/`.error` are omitted from the IPC payload when unset, rather than sent as explicit nulls. The `skip_serializing_if` sat behind `cfg_attr(not(feature = "ts-export"))`, and because cargo unifies features across a build and one test package enables that feature, which form the GUI's JavaScript received depended on what else was in the build graph. Anything reading these fields should treat absent and null alike. The generated TypeScript is unchanged — `ts(optional)` already described the field as optional.

### Changed

- macOS and Windows honour `HOME` and `XDG_CONFIG_HOME` / `XDG_DATA_HOME` / `XDG_STATE_HOME` when they are set to an absolute path, falling back to the platform's native location — `Library/Application Support`, `Library/Logs`, `LOCALAPPDATA` — when they are not. `dirs` reads those variables on Linux alone, and on Windows resolves through the known-folder API, which no environment variable can redirect; the bridge's own paths were therefore impossible to relocate on either platform, which is also why its macOS and Windows test suites had been asserting against the real user profile. Linux behaviour is unchanged. The practical consequence is on Windows, where `HOME` is not normally set but a git-bash or MSYS shell may set it: there the bridge now follows it instead of `%USERPROFILE%`. An empty or relative value is ignored rather than resolved against the working directory.

## [0.19.0] - 2026-07-27

### Fixed

- An embedded GUI asset whose extension the server does not recognise is served as `application/octet-stream`. The fallback was `text/html; charset=utf-8`, so an unrecognised asset was handed to the webview as markup. Only names in the generated `WEB_TEXT_ASSETS` manifest reach the fallback today, so this is hardening rather than a reachable defect.
- GUI asset responses carry `x-content-type-options: nosniff`, which they omitted entirely.

### Changed

- Asset content types resolve through `systemprompt_models::mime` rather than a local table, so the GUI names types the same way the server does. JavaScript is served as `text/javascript` rather than `application/javascript`.

### Added

- Linking a device provisions the registered hosts automatically: every host is probed, and each one that has its app installed gets a profile generated, installed, and synced. Previously the link finished and left nothing installed, so the app stayed unusable until the user found the agents tab and ran the install by hand. Progress appears per host in the setup wizard, which refuses to finish while a run is in flight, and a host that fails is reported rather than passed over silently. A run is recorded in `first-run.json` in the bridge metadata directory, so signing out and back in does not repeat it; `auth clean` removes the sentinel along with the rest of the machine state. A run that stops making progress — an unanswered elevation prompt, a probe that never reports — times out after five minutes and hands the app back with the failures on screen.

## [0.18.0] - 2026-07-24

### Added

- A white-label build sets `SYSTEMPROMPT_BRIDGE_WINRES=off` to skip core's Windows resource embed, so the brand's icon and version info are the only `.rsrc` linked instead of a duplicate pair.

### Changed

- The GUI asset routing table is generated at build time from the staged web tree (core plus brand overlay), so every stylesheet, script, and locale file that exists is embedded and served. A brand overlay can now add new files, not only override existing ones.

### Fixed

- The GUI no longer opens to a blank window when a client module ships without a matching routing entry; the session service module was unroutable, and its failed import prevented the entire client from loading.

## [0.17.0] - 2026-07-21

### Breaking

- **Breaking:** removed the `Brand::synthetic_plugin_name` field. Migrate by deleting it from any custom `Brand` definition; managed plugins now keep the ids the gateway assigns.
- **Breaking:** added the `Brand::schedule_label`, `Brand::schedule_unit`, and `Brand::schedule_task_name` fields (the launchd label, systemd unit basename, and Task Scheduler task name for the periodic sync job). Migrate by adding them to any custom `Brand` definition so a white-label build does not register an upstream-named task.
- **Breaking:** added the `Brand::workspace_dir_name` field (the brand's default Cowork workspace folder name; empty string ⇒ emit no default folder). Migrate by adding it to any custom `Brand` definition.

### Changed

- The OAuth `client_secret` keystore moves from `keyring` 3 to `keyring-core` 1 with explicit per-platform stores (Keychain on macOS, Credential Manager on Windows, the blocking dbus Secret Service on Linux). The `keyring` 4 facade was not adopted: it hard-codes the async zbus Secret Service backend on Linux, and its lazy bootstrap overwrites the process credential store, which would silently replace the headless store the bridge test suites install. Behaviour on all three platforms is unchanged.
- `ed25519-dalek` moves from 2 to 3 and `toml` from 0.9 to 1.1. The Ed25519 wire format is unchanged, so manifests signed by earlier releases still verify.
- The Cowork session directory is resolved deterministically — configured value, then the deployment's personal-session UUID, then a sole usable candidate — and fails loudly listing the candidates instead of guessing the most recently modified one.
- Managed plugins from the gateway manifest are each installed as a distinct plugin in Claude Code and Claude Cowork — carrying their own name, skills, and agents — so the host UI lists one entry per plugin instead of a single merged entry. Managed MCP servers are attached per plugin through the local proxy.

### Added

- `[cowork] session_org_dir` in `systemprompt-bridge.toml` pins which Cowork session/organization directory the bridge syncs into.
- `install --apply-schedule` registers the periodic sync job with the host scheduler (launchd, Task Scheduler, or a systemd user timer) instead of only writing a template for the user to install by hand. Registration is idempotent, the identifiers are brand-scoped, and `uninstall` deregisters the job.
- Windows MDM policy pre-trusts a default Cowork workspace folder (`allowedWorkspaceFolders` → `~/<brand workspace dir>`, surfaced as a default-selected folder chip) and materializes the directory on apply, so the agent gets a real writable working directory instead of wandering into protected host paths and triggering folder-permission prompts. The policy also pins `coworkEgressAllowedHosts` to loopback and disables `isLocalDevMcpEnabled`.
- The integration layer reports a per-host application launch state, which the tray and the hosts list render, so a host that is installed but not running is distinguishable from one that is unavailable.
- A web client session service backs the rail profile, cloud status, and setup views from one source of truth instead of each view fetching its own.

### Fixed

- `managedMcpServers` is now written to `HKLM\SOFTWARE\Policies\Claude` on Windows: Cowork ≥ 1.22209 ignores the `HKCU` policy hive entirely when an `HKLM` policy exists, so the previous `HKCU` write left Cowork loading zero managed servers. An unelevated run clears the ignored `HKCU` copy and no-ops when a stable `HKLM` value already exists, erroring only when policy was never provisioned elevated.
- The host-sync registry now dedups emitters by concrete type instead of `host_id`, so the two Cowork emitters that deliberately share the `cowork` host id (plugin enables + the artifacts library) both run again; previously one was silently dropped.
- Sync prunes plugins Cowork still holds in its own copy of the org-provisioned marketplace after the manifest dropped them. Cowork installs each plugin into its own tree and never removes an orphan, so the retired `systemprompt-managed` aggregate kept appearing in its plugin picker.
- An unelevated run compares the desired `managedMcpServers` value against the live `HKLM` policy and requests elevation when they differ, instead of treating any existing value as current. A managed MCP server added after the policy was first provisioned never reached Cowork's connector list. A matching value still no-ops, so a steady-state sync raises no elevation prompt.
- The GUI host dedupe no longer suppresses real changes. The client's volatile-key set omitted `expires_at_unix`, so a host payload carrying it compared unequal on every probe tick, and the semantic hash mixed in no variant tag, letting an object and an array with equal leaf content collide and hide a genuine change.
- The GUI activity log survives re-render. Each render replaces the component's `innerHTML`, detaching the nodes the virtual list was bound to, and the list was only ever built once, so the log went permanently blank while the header counters kept updating.

### Removed

- The single aggregate `systemprompt-managed` plugin that combined every managed skill and agent into one host entry, along with its reserved-id guard.

## [0.16.0] - 2026-07-03

### Added

- Cowork artifacts emitter: the manifest's `artifacts` section is materialised through two sinks — a staging directory consumed by the first-run `create_artifact` seed skill, and an on-disk library store — with content-hashed idempotency and remove-on-empty cleanup.
- The GUI marketplace gains an Artifacts category listing the entries in the local library store.

## [0.15.0] - 2026-06-25

### Added

- One-click browser sign-in on the setup splash: the bridge opens the gateway's device-link consent page and completes authentication without a manual code paste.
- Durable bridge sessions: the one-time exchange code is swapped for a long-lived personal access token, so the bridge survives restarts without re-running device link.
- `doctor` reports hosts whose installed loopback secret is out of date (installed fingerprint no longer matches the live proxy secret) and prints the re-apply remediation.
- The MCP registry rehydrates from its on-disk snapshot at startup, so the proxy can serve `/mcp/<name>` immediately after launch instead of waiting for the first credentialed sync.

### Changed

- Sensitive values read from host configurations (gateway API keys, loopback secrets) are redacted to a short fingerprint before they reach diagnostics, logs, or the GUI; raw secrets are never surfaced.
- Loopback-secret rejection is split by cause: an unauthenticated caller (no bearer) logs at `debug`, while a stale-secret mismatch logs at `warn` with a remediation hint and an activity-log line. The forbidden response no longer implies the caller is malicious.

## [0.14.0] - 2026-06-22

### Added

- White-label builds are now supported through a compile-time brand seam. A downstream binary crate can supply its own application name, on-disk directories, environment-variable prefix, default gateway URL, keyring service, window/tray chrome, and GUI assets (including a theme stylesheet layered last in the page `<head>`) by installing a `Brand` at process start via `run_with_brand`. The default `systemprompt` binary is unchanged.

### Changed

- The application name, configuration and state paths, environment-variable prefix (`SP_BRIDGE_*`), default gateway URL, keyring service, device-link consent path, and all user-facing command hints are resolved from the active brand rather than hardcoded, so a rebranded build presents its own identity consistently across the CLI, GUI, logs, and generated profiles.
- Log line prefixes and diagnostic bundle names are derived from the active brand's binary name.

## [0.13.0] - 2026-06-09

### Added

- A headless `proxy` subcommand runs the local inference proxy without the desktop GUI — the Linux/server equivalent. It listens on `127.0.0.1:48217`, swaps a loopback secret for a fresh gateway JWT, injects the identity headers, and refreshes the token in the background; point `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` at the printed values.

## [0.12.1] - 2026-06-04

### Fixed

- The synthetic managed-plugin write is now idempotent against content. The plugin's `version.json` is derived from a hash of the bundle (skills, agents, hooks) rather than the gateway's per-poll manifest version, and is written last as the completion marker the next sync's skip check keys on. An unchanged bundle therefore leaves the plugin directory byte-for-byte untouched instead of being removed and rewritten, so Claude Code no longer momentarily drops or re-installs the managed plugin on every poll.

## [0.12.0] - 2026-06-03

### Added

- The bridge mirrors an organization's managed plugins into the standalone Claude Code CLI. Because the `claude` CLI does not read the Cowork org-plugins root, the bridge now installs the managed skills, agents, and MCP servers into `~/.claude` as a directory-source marketplace plugin — writing the bundle, `marketplace.json`, and the `known_marketplaces` / `installed_plugins` registry entries, then force-enabling it in `settings.json` — so the plugin appears in `claude plugin list` and its skills load as `/systemprompt-managed:<skill>`. Every registry file is updated in place, preserving the user's other marketplaces and plugins, and a manifest with no content removes the plugin again.

## [0.11.0] - 2026-06-03

### Added

- Per-host compatible-model selection in the Status tab. Each managed host now exposes which wire protocol(s) it advertises, and the user can pin a host to a subset (or clear the override to fall back to the host's default). The choice is persisted through the gateway, carried back in the signed manifest as `host_model_protocols`, and applied when generating the host's policy profile so the host is offered only the models its client can drive. An empty selection means "all models".

### Fixed

- A profile fetch made while signed out is treated as the expected logged-out state on the login page instead of an error: no error log line and no toast. The handler now recognises a dedicated not-authenticated result and renders the logged-out view quietly.

## [0.10.8] - 2026-06-03

### Added

- A managed host's policy profile now carries an `inferenceCustomHeaders` entry that pins the host's wire protocol on every inference request (`x-inference-protocol`). The gateway uses it to scope the advertised model list to the protocol the host's client actually speaks, so a single shared gateway offers Claude Desktop its Anthropic models and Codex CLI its OpenAI models rather than handing every host the same flat list. A host that accepts no specific protocol sends no extra headers. The header is emitted by all three Claude Desktop profile forms (macOS `.mobileconfig`, Windows `.reg`, and the in-process registry write).

## [0.10.7] - 2026-06-03

### Added

- The Status tab now lists each host's compatible models in a dedicated **Compatible models** row, so it is clear up front which models a host can actually drive rather than leaving model selection to guesswork. The set is derived from the gateway's `/v1/bridge/profile` provider health and filtered to the host's wire protocol.
- A host whose only matching provider has no usable model now shows a **"no compatible model"** badge instead of reporting healthy, and the card explains why — naming the provider(s) missing an API key when that is the cause. A host that has not yet been checked (e.g. the gateway was unreachable) is kept distinct from one with nothing usable, so the warning never fires on startup before any health is known.
- The marketplace listing includes hooks synced from plugins (`hooks/hooks.json`): the managed govern/track entries collapse into a single summary row while user-defined command hooks are listed individually.

### Changed

- A managed host is offered only the models whose wire protocol it speaks: Claude Desktop receives Anthropic models, Codex CLI receives OpenAI models. Previously every host received the same flat model list, which could hand a host models its client cannot use. The filtered set drives both the generated host profile and the GUI's per-host model display.

### Fixed

- Installing the Claude Desktop managed-policy profile now elevates on demand instead of failing on a standard account. The policy lives under `SOFTWARE\Policies\Claude`, an ACL-protected subtree that a non-elevated token cannot create in either hive, so the in-process write introduced in 0.10.6 returned `ERROR_ACCESS_DENIED` (status 5) for every unprivileged install — including the `HKEY_CURRENT_USER` fallback. When the bridge is not already elevated it now relaunches its own executable under a Windows UAC consent prompt to write the policy machine-wide (`HKEY_LOCAL_MACHINE`); the activity log explains the prompt before it appears, a declined prompt surfaces an "administrator approval was declined" message instead of a raw status code, and an access-denied write now reports which hive and subkey require administrator rights.

## [0.10.6] - 2026-06-02

### Changed

- Installing the Claude Desktop managed-policy profile no longer shells out to `reg import`. The install path parses the staged `.reg` profile and writes each policy value directly through the Windows registry API (`RegCreateKeyExW`/`RegSetValueExW`), choosing `HKEY_LOCAL_MACHINE` when elevated and `HKEY_CURRENT_USER` otherwise, which removes the dependency on an external binary and surfaces a structured error on failure. The `.reg` render and parse halves move to a platform-independent module so the round-trip is unit-tested on every target.

## [0.10.5] - 2026-06-02

### Fixed

- The Status tab's **MCP servers** section badge no longer stays "UNKNOWN" when a server is authenticated. `sp-mcp-auth-status.js` seeded the section rollup with `"unknown"`, which `rollUp` ranks above `ok`, so an authenticated server could never lift the badge to green; the section state is now derived from the servers' worst state.
- The MCP auth probe now captures the backend's `Mcp-Session-Id` from `initialize` and surfaces it on the server card (`McpServerAuth.session_id`), confirming a session was established and aiding session-lifecycle debugging.

## [0.10.4] - 2026-06-02

### Fixed

- The setup wizard's **Finish** button is no longer gated on `anyInstalled`. Host install-state is probe-driven and can lag or misreport (the agent card shows "Installed ✓" while the parent's `anyInstalled` flag is still `false`), which trapped the user on step 2 with a permanently disabled Finish and no way into the app. Finish is now always enabled — installing agents is optional.

## [0.10.3] - 2026-06-02

### Fixed

- The proxy's upstream client (`proxy::server::build_upstream_client`) now installs the same `Ipv4FirstResolver` the gateway client already uses. The client forwards Cowork's MCP and `/v1/messages` inference to the gateway, so when a user configured the gateway as `http://localhost:…`, every proxied call resolved IPv6 `::1` first and stalled the full connect timeout (~15-21s) before falling back to IPv4 — the WSL2 localhost forwarder black-holes IPv6 SYNs. Sync/probe/profile-fetch were already IPv4-first via the gateway client; the proxy path was the remaining gap, so a user-entered `localhost` URL no longer freezes proxied traffic. `gateway::Ipv4FirstResolver` is now `pub(crate)`.

## [0.10.2] - 2026-06-02

### Added

- **MCP authentication status in the GUI.** The Status tab gains an "MCP servers" group that runs a live `initialize` → `tools/list` round-trip per registered server through the loopback proxy (`proxy::mcp_probe`) and classifies the result — Authenticated, `bad loopback secret` (403), gateway unauthorized (401), proxy unreachable, etc. — so failures that previously required reading Cowork's `main.log` are visible in-app. Authenticated servers list the tools they expose as chips. The panel re-probes automatically after each sync and via a manual "Recheck" button. The MCP server's tools are also listed in the Marketplace detail view.

### Changed

- The synthetic org plugin now ships `installationPreference: "required"` (was `"auto_install"`). `auto_install` installs once at sign-in but treats a later removal as a sticky user-uninstall, so a cleared install record never returned and the plugin sat behind a manual "Add" with its skills disabled. `"required"` is the org-plugin equivalent of the `managedMcpServers` policy — it force-installs at every sign-in and reinstalls if removed — so skills/agents/hooks land automatically, matching the managed MCP connector. Takes effect on each user's next Cowork sign-in.

### Fixed

- `gateway::Ipv4FirstResolver` no longer uses a trivial `as` cast to box its address iterator (a `Box<…> as Box<dyn …>` unsizing the newer toolchain's `trivial_casts` lint flags); the coercion is now expressed via a typed binding.

## [0.10.1] - 2026-06-02

### Fixed

- Windows: `managedMcpServers` (which embeds the rotating loopback secret) is now written only to the per-user `HKCU\…\Policies\Claude` hive — the same hive the GUI already owns for `inferenceGatewayApiKey` — instead of the machine-wide `HKLM` MDM policy. Pinning the secret in HKLM was a latent split-brain bug: once the secret rotated under a non-elevated bridge run (which cannot rewrite or delete HKLM), the stale HKLM value outranked HKCU, so Cowork connected with the old secret and failed every MCP call with `forbidden: bad loopback secret`. `managedMcpServers` is dropped from `windows_policy_values` (the HKLM policy now carries only stable, secret-free keys), and the writer best-effort purges any stale HKLM copy left by older builds.
- Windows: `bridge --uninstall` now actually clears the managed registry policy — `remove_managed_profile` was a no-op on Windows, so a stale, secret-bearing `managedMcpServers` survived "clean" reinstalls. It now removes the bridge-owned `HKCU\…\Policies\Claude` key and best-effort deletes `HKLM\…\managedMcpServers`, so a reinstall starts from a clean registry.

## [0.10.0] - 2026-06-01

### Changed

- The synthetic organization plugin's `plugin.json` and the malformed-plugin / well-formedness probes use the shared `PluginManifest` model and manifest-path constants from `systemprompt-models::bridge::plugin_bundle` instead of bridge-local copies. The emitted wire shape is unchanged.

### Added

- The bridge detects whether each host's desktop app is installed, launches or focuses it on request, and offers an official download link when the app is absent. `HostAppSnapshot` gains `app_installed`, the `HostApp` trait gains `download_url()`, and the setup UI surfaces install, launch, and download actions.

## [0.9.6] - 2026-05-30

### Changed

- Plugin hook calls route through the bridge loopback proxy instead of the gateway directly. The per-plugin `hooks.json` now points Cowork at the proxy's loopback URL with the static loopback secret as `Authorization`; the proxy verifies and strips that header, mints the plugin's `aud:hook` gateway token (resolved from the `plugin_id` query parameter), and injects it before forwarding to the public hook endpoints. This replaces the per-plugin `.env.plugin` file and the `$SYSTEMPROMPT_PLUGIN_TOKEN` env-var substitution, which Cowork's agent VM did not reliably propagate into the hook subprocess; `allowedEnvVars` is now empty. A hook-route `401` rotates the per-plugin hook token rather than invalidating the shared bridge token cache.
- Hook-scoped credentials issued by `admin keys issue-plugin-token` no longer carry the minting admin's roles. A hook token (`aud:hook`) authorizes on scope and `plugin_id` only, so the roles were inert.
- The GUI marketplace lists managed MCP servers from the in-memory MCP registry — the same source that feeds the `managedMcpServers` policy — rather than the removed synthetic-plugin `.mcp.json`.

## [0.9.5] - 2026-05-29

### Changed

- Managed MCP servers are registered with Cowork through the bridge's loopback proxy rather than the upstream gateway. Each entry points Cowork at the proxy's loopback URL with a static loopback-secret `Authorization` header instead of carrying `oauth: true`; the proxy strips that header and injects the rotating, auto-refreshed gateway JWT before forwarding to the registered upstream. This sidesteps Cowork's OAuth flow entirely — it hard-rejects the gateway's non-HTTPS authorize URL on Connect — while every request still carries a live token. Applies to both the MDM managed-prefs writer (`install::mdm`) and the synthetic-plugin writer (`sync::apply::synthetic_plugin`); when the loopback secret is unavailable the managed server list is emitted empty rather than half-configured.

## [0.9.4] - 2026-05-28

### Breaking

- `bridge::manifest::AgentEntry.mcp_servers` and `AgentEntry.skills` are now `PluginComponentRef { source, include, exclude }` instead of `Vec<String>`. The manifest envelope tracks the unified `PluginComponentRef` shape now applied across every entity-id reference list in `systemprompt-models`. Bridge / Cowork consumers that read these fields must traverse `.include` instead of treating the value as a flat list; serialised manifests authored against 0.9.3 are no longer accepted.

### Changed

- Bridge no longer emits the `deploymentOrganizationUuid` policy key into the Claude Desktop managed-prefs plist (macOS) or `HKCU\…\Policies\Claude` registry hive (Windows). Cowork's 3P custom-gateway contract is inference-only (`POST /v1/messages` + optional `GET /v1/models`, per docs/cowork/3p/gateway and gateway-sso); a custom gateway has no spec surface to assert the `integrations:manage` permission that this key flips Cowork into checking. Emitting it locked the Install button under the "Contact an organization owner to install connectors" tooltip without recourse. Cowork now resolves through `manageFromPersonal = true` and the Install button is live — MCP installation and use over the bridge proxy are unchanged.
- `pick_target` no longer takes a `policy_uuid` argument and `resolve_target` no longer reads the now-absent `deploymentOrganizationUuid` policy key; Cowork plugin sync resolves the personal-session org dir directly, falling back to newest-mtime when the personal dir is missing.
- Bridge keeps its own working state (`.staging/`, sync sentinel, version sentinel, user fragment) under a platform-specific user-writable directory (`%LOCALAPPDATA%\systemprompt-bridge` on Windows, `~/Library/Application Support/systemprompt-bridge` on macOS, `$XDG_STATE_HOME/systemprompt-bridge` on Linux) instead of nesting them under the published `org_plugins` tree. On Windows that tree lives under `Program Files` and is admin-write-only, so writing scratch state inside it raised `Sync failed: io error in create staging: Access is denied` for non-elevated bridge runs. `paths::metadata_dir(_)` / `paths::staging_dir(_)` and the `METADATA_DIR` / `STAGING_DIR` constants are gone; callers use the new `paths::bridge_working_dir()` / `bridge_staging_dir()` / `bridge_metadata_dir()`.

### Added

- `bridge doctor` adds a `hook token mint` check that exchanges the cached OAuth client credentials for a hook token against the gateway's token endpoint with `plugin_id=__doctor__`. Failures surface the gateway's `error_description` verbatim on a single line instead of waiting for the next `sync` PARTIAL output.
- `bridge doctor` adds a `personal-session sentinel` check that scans Cowork's sessions root for an org dir matching `PERSONAL_SESSION_UUID` (`00000000-0000-4000-8000-000000000001`). If Cowork sessions exist but none matches, the constant has drifted from Cowork's source of truth and `pick_target` will silently fall through to its mtime fallback — the check fails loud so the operator updates the bridge before sync misroutes plugins into the wrong session.

## [0.9.3] - 2026-05-28

### Changed

- `marketplace.json`, `known_marketplaces.json`, and `installed_plugins.json` are written in the shape the current Cowork (Claude 1.5354) reader expects: `marketplace.json` gains `$schema`, `description`, `metadata { description, version, pluginRoot }`, and per-plugin `author`/`category`, with `plugins[].source` flattened to a plain string path; `known_marketplaces.json` is a top-level object keyed by marketplace name with `source`, `installLocation`, and `lastUpdated` per entry; `installed_plugins.json` is `{ "version": 2, "plugins": { "<plugin>@<marketplace>": [{ "scope", "installPath", "version", "installedAt", "lastUpdated" }] } }`. Foreign sibling entries continue to be preserved verbatim.
- Cache and marketplace path joins sanitise version strings before writing to the filesystem; RFC3339-shaped versions containing `:` no longer trip Windows ERROR_INVALID_NAME during `bridge sync`.
- `sync` propagates per-host emit failures into `SyncSummary::host_failures` and the one-line summary now reads `sync PARTIAL (…) — N host(s) failed: …`, so a silently half-published marketplace surfaces in the GUI Activity panel instead of being reported as `sync ok`.
- 403 "bad loopback secret" rejections log the resolved secret path, and `tracing` lines on empty / missing / freshly minted secret files include the file path, giving operators a single line to follow when Claude Desktop has cached a stale loopback secret.
- `GatewayError::HookTokenRejected { status, body }` replaces the bare `HttpStatus` mapping for `mint_plugin_hook_token` non-2xx responses; the gateway's error body is preserved so `bridge sync` PARTIAL lines carry the underlying RFC 6749 §5.2 reason instead of an opaque status code.

### Added

- `bridge doctor` command groups the bridge-side self-checks (config, credential source, mint JWT, gateway reachable, authenticated whoami, loopback secret, pinned pubkey, cowork marketplace registration) into a single one-line-per-check diagnostic surface; exits 11 on any failure.
- `SyncError::GatewayUnauthorized { endpoint, status }` represents gateway 401/403 from `/manifest` and `/pubkey` as a distinct error with exit code 10 and an actionable "run `systemprompt-bridge login <sp-live-...>`" message; the GUI surfaces it via the new `sync-gateway-unauthorized` Fluent string, and the `sync-no-credentials` string handles the no-PAT-configured case.
- Typed wire-shape structs for the Cowork host adapter: `KnownMarketplacesFile`, `KnownMarketplaceValue`, `InstalledPluginsFile`, `InstalledPluginInstall`, and `MarketplaceMetadata`, replacing the ad-hoc `serde_json::Value` traversals.
- Unit test coverage for the Cowork host adapter (`crates/tests/unit/bridge/cowork-plugins`): canonical marketplace shape, known-marketplaces / installed-plugins / settings upsert behaviour, and path sanitisation.

## [0.9.2] - 2026-05-27

### Changed

- Track `systemprompt-identifiers` and `systemprompt-models` 0.12.0 dependency pins.

## [0.9.1] - 2026-05-25

### Changed

- **Internal lint and visibility cleanup.** Bridge sources adopt the workspace's tightened clippy baseline (`unreachable_pub`, `allow_attributes_without_reason`, `redundant_pub_crate`, `let_underscore_must_use`) — visibility narrowed from `pub` to `pub(crate)` where appropriate, MDM helpers cfg-gated to the OSes that consume them, best-effort `Result` discards justified with `tracing::warn!`. No user-visible behaviour change.

## [0.9.0] - 2026-05-22

### Fixed

- **Session binding: bridge persists and binds its stable `x-session-id`.** The bridge now stores its `x-session-id` and replays the same value across requests, so `/v1/messages` and `/bridge/heartbeat` no longer return `401 "Session missing or revoked"` or `"X-Session-ID does not match"` after the first call. A regenerated session id per request previously orphaned the gateway-side session record.

### Added

- **`HostSync` trait + central dispatcher (`sync/host_sync.rs`).** Every bridge integration that materialises manifest data on disk (Cowork synthetic plugin, Codex managed resources, Windows MDM, …) implements one `HostSync` trait with `apply` / `clear` methods. The dispatcher in `sync::apply` walks the static `registry()`, decides per-host whether to call `apply` or `clear` based on the manifest's `enabled_hosts` field, and uniformly logs each outcome — emitter authors no longer reinvent the toggle-and-cleanup gate. Replaces the imperative pile of "call `cowork::publish` then `mdm::reconcile` then …" in `sync::apply::mod`.
- **Codex CLI host emitter (`integration/codex_cli/managed_resources.rs`).** Implements `HostSync` for Codex by writing a single plugin bundle that matches Codex's documented discovery contract (verified against the published JSON schema and `developers.openai.com/codex/plugins/build`). Skills and MCP servers land as one Codex plugin at `~/.codex/plugins/cache/systemprompt/systemprompt-managed/current/`, containing `.codex-plugin/plugin.json` (carrying the manifest version), `skills/<id>/SKILL.md`, and `.mcp.json`. A `[plugins."systemprompt-managed@systemprompt"] enabled = true|false` block in `~/.codex/config.toml` is the user-facing toggle; every other key in `config.toml` (user MCP servers, sibling plugins, model providers) is preserved across `apply` and `clear`. Earlier iterations wrote to `~/.codex/skills/` and to top-level `[mcp_servers.sp_*]` blocks — neither path is read by Codex, so the marketplace bundle was invisible inside the CLI.
- **Codex provider-profile install (`integration/codex_cli/install.rs`) targets the documented system path and merges instead of overwriting.** Linux/macOS now write to `/etc/codex/config.toml` (the prior `/etc/codex/managed_config.toml` was undocumented and not in Codex's config chain). The install reads the existing target, strips bridge-owned keys (`model_provider`, `model_providers.systemprompt`, `otel`, `analytics`), deep-merges the freshly generated TOML on top, and atomic-writes — so prior keys survive reinstall. New `CODEX_SYSTEM_CONFIG` env var overrides the system path for hermetic tests.
- **GUI: per-host enable toggle posts to gateway (`gui/handlers/agents.rs::on_set_enabled_host_requested`).** New IPC entrypoint sends `POST /v1/bridge/enabled-hosts` with the host id and desired state, then emits `UiEvent::SetEnabledHostFinished`. The GUI no longer mutates local `agents.json` directly — host enable state is a profile fact owned by the gateway and arrives back through the next signed manifest. Matches the broader rule that host enable state lives in the user profile, not local toggles.

### Changed

- **`integration/codex_cli/install.rs` (326 lines, hand-rolled base64) split into `install/{mod,merge,render}.rs`.** `mod.rs` keeps `write_profile` / `install_profile` / `writable`; `merge.rs` owns `merge::install` plus the `OWNED_*` constants for bridge-owned keys; `render.rs` owns TOML + mobileconfig rendering. The 93-line `render_managed_toml` is now 16 lines, dispatching to `write_provider_block` / `write_otel_block` / `write_models_block`. Hand-rolled `base64_encode` replaced with `base64::engine::general_purpose::STANDARD`. WHAT-doc-comments on `OWNED_*` consts collapsed into a single module-level `//!` block.
- **Silent error sites in `sync/mod.rs::persist_last_sync` and `integration/codex_cli/probe.rs::parse_into_keys` now log via `tracing::warn!`.** Three `let _ = …` / `.unwrap_or_default()` discards in `persist_last_sync` and one `.ok()?` on TOML parse in `probe::parse_into_keys` previously dropped errors silently; each now logs context (path, dir, source) before the best-effort fallback.
- **Bridge codex tests no longer use `unsafe { env::set_var }`.** `crates/tests/unit/bridge/{sync,integration}/src/codex_*` rewritten on top of the `temp-env` crate (added as workspace dev-dep) — each test scopes `CODEX_HOME` / `CODEX_SYSTEM_CONFIG` via `temp_env::with_var(s)` instead of mutating process env, removing the manual `Mutex<()>` lock and the `unsafe` block.
- **`agents_state` simplified.** `migrate_from_existing_profiles` (which probed every registered host on startup) and `store_exists` are gone. Replaced by `save_from_manifest(enabled_hosts: &[String])`, called from `sync::apply` whenever a new signed manifest is applied. `save` is now `pub(crate)`. The first-run "auto-enable everything that looks installed" migration is no longer needed because the manifest is authoritative.

### Added

- **Cowork plugin sync (`integration/cowork_plugins/`).** Per-plugin marketplace publish into the active `<session>/<org>/cowork_plugins/` tree: marketplace upsert, installed-plugin upsert, enabled-settings upsert (foreign-entry preservation throughout), plus a per-plugin `claude-plugin/plugin.json` patch that wires `hooks/hooks.json`. Reverse `unpublish` path included.
- **OAuth hook-token client (`auth/plugin_oauth.rs`).** Per-tenant OAuth client + plugin-scoped hook-token cache. `client_secret` is stored in the OS keystore (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux) via the `keyring` crate; only `client_id`, `token_endpoint`, and `scopes` remain on disk. Legacy 0600 JSON files containing `client_secret` are transparently migrated into the keystore on first read.
- **Typed `hooks.json` schema (`sync/apply/hooks_schema.rs`).** `HooksFile`/`HookEntry`/`HookKind` replace the prior `serde_json::json!` literal in `sync/apply/hooks.rs::write_hooks_json`.
- **`fsutil` module.** Single owner of `atomic_write_0600` (parent dir 0o700, fsync before rename), `copy_dir_recursive`, and `read_optional`. Removes three duplicate implementations across `auth/`, `sync/`, and `integration/`.
- **`mcp_registry` (top-level).** Cross-cutting registry consumed by `proxy::forward`, `install::mdm::*`, and `sync::apply` — relocated from `proxy::mcp_servers` because `proxy::` mis-suggested ownership.

### Changed

- **`gateway/` split.** `gateway/mod.rs` (489 → 79 lines) into `mod` (client) + `errors` + `types` + `fetch` + `auth`.
- **`integration/cowork_plugins/emit.rs` split** (411 → 245 lines) into `emit` (publish/unpublish orchestration) + `upsert` (registry/settings file plumbing). Visibility narrowed: `mod {emit, marketplace, registry, settings}` are now `pub(crate)`; only `KNOWN_MARKETPLACES_FILE`, `publish`, `resolve_target`, `unpublish`, and the test surface stay `pub`.
- **`install/mod.rs` split** (313 → 170 lines) by extracting orchestration glue (`bootstrap_install`, `run_apply*`, `resolve_*`) to `install/apply.rs`.
- **`sync/apply/plugin.rs` split** (322 → 184 lines) by moving `materialize_hook_token`, `write_hooks_json`, and `ensure_plugin_json_hooks_field` to `sync/apply/hooks.rs`.
- **`sync/apply/mod.rs::rewrite_loopback_urls`** uses `url::Url::set_host`/`set_scheme` against `Host::Ipv4`/`Host::Ipv6` loopback checks instead of string-splitting helpers (`split_url`, `split_origin`, `is_loopback_host` deleted).
- **`SignedManifest` family moved to shared crate.** `SignedManifest`, `UserInfo`, `PluginEntry`, `PluginFile`, `SkillEntry`, `AgentEntry`, `ManagedMcpServer`, `ManifestVersion`, plus the manifest-scoped typed IDs (`PluginId`, `SkillId`, `Sha256Digest`, `ManifestSignature`, `ToolPolicy`, etc.) now live in `systemprompt_models::bridge::*`. Bridge re-exports preserve every existing call site; the bridge-side ed25519 `verify(...)` is provided via the new `SignedManifestVerify` extension trait (orphan-rule workaround).

### Fixed

- **Proxy: `/healthz` and `/otel` no longer rejected by the loopback-secret gate (`proxy/server.rs`).** The healthz short-circuit ran *after* the bearer check, and only matched `GET`, so the bridge's own `HEAD /healthz` probe (and any external poller) flooded the activity log with `403 (bad secret; presented_fp=<empty>)` every 30 s. Codex's OTLP-HTTP exporter posting to `/otel` hit the same gate — OTLP has no clean way to inject the loopback bearer. Both paths are now handled by an explicit `is_unauthenticated_path(method, path)` predicate evaluated *after* the loopback-host check and *before* the bearer check: `GET`/`HEAD /healthz` short-circuits in-process; `POST /otel` (and `/otel/*`) forwards through `forward::forward`, which already strips the inbound `Authorization` and injects the upstream bearer from `TokenCache`. Loopback-origin enforcement is unchanged. Same change folds the response building into a single shared `forward_to_gateway` helper, collapses `health_response` to two lines via `simple_response("")`, replaces the unreachable `Response::builder().unwrap_or_else` fallback with infallible `Response::new` + `HeaderValue::from_static`, fixes a multi-strip bug in the bearer parser (`trim_start_matches("Bearer ")` could strip repeats; now `strip_prefix` once), and tightens `sha256_8` and `record_stats` (callers now pass an already-computed `latency_ms` instead of `Instant`).

- **`///` rustdoc and TODO/FIXME flags purged from binary modules** (`bin/bridge/**` is a binary — `///` is banned). ~50 paraphrase blocks removed; ~20 load-bearing why-lines preserved as `//`. The `obs.rs` panic-hook ordering note is retained as a `// Why:` comment; the `gui/server.rs` focus-IPC FIXME was reworded as a deliberate-trade-off explanation (TCP+CSRF works identically across all three platforms in <100 lines).

- **Breaking — `cowork` rename completed end-to-end.** Bridge sends canonical `x-session-id` / `x-context-id` headers (issued from the new `SessionContext`) and uses the renamed gateway routes (`/v1/bridge/*`, `/v1/auth/bridge/*`). Internal macros are now `bridge_define_id!` / `bridge_define_token!`. Env vars: `SP_COWORK_*` → `SP_BRIDGE_*`. Config file: `~/.config/systemprompt/systemprompt-cowork.toml` → `systemprompt-bridge.toml`. A `0.7.x` bridge cannot talk to a `0.8.0` gateway and vice versa.

### Added

- **Heartbeat loop (`proxy/heartbeat.rs`).** Spawned next to the token-refresh loop in `proxy/server.rs::start`; POSTs `/v1/bridge/heartbeat` every 30 s with `session_id`, `bridge_version`, OS, hostname, `last_activity_at`, and a snapshot of `ProxyStats` (forwarded count, tokens in/out). The gateway records the row in `bridge_sessions`, making this bridge visible to `systemprompt admin bridge list` even between inference requests. On `401` the token cache invalidates so the next tick re-authenticates.
- `SessionContext::touch_activity()` is called on every successful messages-path forward, so the heartbeat distinguishes "alive but idle" from "alive and serving traffic".
- Bridge sends canonical `x-session-id` and content-derived `x-context-id` headers on every `/v1/messages` forward, enforcing conversation grouping at the gateway.

### Fixed

- **Tech-debt sweep on the per-agent enabled feature.**
  - `auth::setup::clean()` now also removes `~/.config/systemprompt/agents.json`. Previously a `clean` left stale enabled state behind.
  - Existing users get a one-shot migration on first run after upgrade: when no `agents.json` exists yet, `gui::run_agents_migration_if_needed` probes every registered host and auto-enables those whose `profile_state` is already `installed`. The old "everything is silently disabled" behaviour after upgrade is gone.
  - `apply_host_snapshot` no-ops (and removes any existing entry) when the host has been disabled mid-probe, so an in-flight probe that finishes after a disable can no longer re-insert the host into `state.hosts`.
  - `agents.setEnabled` is now idempotent: setting the same value twice returns `{ changed: false }` and skips both the activity-log line and the wasted manual probe.
  - Setup-wizard "Install profile" handler now records which step failed (`enable` / `generate` / `install`) on the button's `data-failed-stage` and surfaces the underlying error message in `title`, so partial failures stop being silent.
  - `proxy_probe::probe` does an actual HTTP `HEAD /healthz` after the TCP connect and reports the status on `ProxyHealth.http_status`, so a stray process listening on port 48217 no longer claims `Listening` for the bridge proxy.
  - Renamed `GatewayClient::fetch_cowork_profile` → `fetch_bridge_profile` to finish the cowork→bridge rename on the bridge side. Server endpoint path and `CoworkProfile` type are unchanged (server contract).
  - Moved `agents_state` from `gui/` to a top-level module so non-GUI builds (`auth::setup::clean`) can reference it without `cfg`-gates.
- **Setup wizard's Install button was a no-op.** It only called `host.profile.generate`, which writes a profile file but does not copy it into the OS-managed location, so the host's `profile_state` never flipped to `installed` and the UI stayed on "Install profile" forever. Now the setup-agents handler enables the host (so the new gating doesn't reject the call), generates the profile, and immediately installs the resulting path — three IPCs in sequence — matching the user's intent of "set up this agent".
- **Local proxy probe always returned `Unconfigured` until at least one host had been installed**, because `AppState::first_configured_proxy_url` derived the probe target from `host.profile_keys.inferenceGatewayBaseUrl` (which only populates after install). The bridge owns the proxy and knows its port — now `first_configured_proxy_url` returns `http://127.0.0.1:<proxy.handle.port>` when the proxy is running, falling back to the host-derived URL only if the proxy hasn't started yet. Cures the "awaiting first launch" badge that stuck even when Claude was actively routing through the proxy.

### Added

- **Per-agent enable/disable, persisted across runs.** Every registered host (Claude Desktop, Codex CLI, …) now has an explicit `enabled` flag stored in `~/.config/systemprompt/agents.json`. Hosts default to **disabled** so a fresh install never silently probes integrations the user hasn't opted into. New IPC `agents.setEnabled({ hostId, enabled })` toggles the flag, persists it, and (when re-enabling) fires a one-shot manual probe. The host card grows an Enable/Disable button; disabled cards render as a dimmed lede with the toggle and no action buttons. `host.probe`, `host.profile.generate`, `host.profile.install`, `agent.uninstall`, and `agent.openConfig` reject disabled hosts with `Conflict`. Status summaries and the rail's agent count consider only enabled hosts.

### Fixed

- **Codex (and every other registered host) was probed every 30 s even when not installed**, spamming the activity drawer with `[codex-cli] re-verifying profile and process` / `re-verify complete — profile not installed, process not running` pairs forever. Two changes: (1) the periodic ticker (`gui/hosts/tick.rs`) now skips hosts that aren't `enabled`, and (2) tick-driven probes are silent in the activity log unless the snapshot's `profile_state` *kind* or `host_running` actually flipped, in which case a single `[host] state changed — …` line is appended. User-triggered Re-verify clicks keep the existing verbose `[host] re-verifying…` / `re-verify complete — …` pair via a new `ProbeCause::{Tick,Manual}` enum threaded through `HostUiEvent::Probe{Requested,Finished}`.
- **Sync failed with no visible reason — only the literal string `sync-failure` in the activity drawer.** `i18n::t_args("sync-failure", ...)` and `i18n::t("sync-cancelled")` had no matching keys in `web/i18n/en-US/bridge.ftl`, so the fallback returned the bare key and the underlying error string was discarded. Added `sync-failure = Sync failed: { $error }` and `sync-cancelled = Sync cancelled.`, switched the error formatter in `gui/handlers/sync.rs` to `{:#}` to print the chain, and added `tracing::info!` / `tracing::error!` so the same message lands in the log file as well as the UI.
- **Sync hard-failed on a redundant directory-level hash check.** After every per-file SHA-256 (signed by the gateway) was verified, `sync/apply/plugin.rs` re-hashed the staging directory with `directory_hash` and compared against `plugin.sha256`. The bridge's hash algorithm did not match the gateway's, so the staged-vs-manifest comparison always failed (`hash mismatch for plugin enterprise-demo: expected …6930, got …495a`), aborting the entire sync. Removed the directory-level check and the now-dead `directory_hash` / `collect_files` helpers from `sync/hash.rs`. Per-file hash verification on a fresh staging dir already guarantees byte-identical contents from the signed manifest.
- **External link clicks could fail silently.** `gui/window/mod.rs::open_target` discarded the `Command::spawn` result, so when `xdg-open` / `cmd /C start` was missing or failed there was no record. Now logs the attempt at info level and the spawn error at error level.
- **Footer links now open via an explicit IPC instead of `target="_blank"`.** Added an `openExternalUrl` IPC command in `gui/command.rs` (HTTPS-only allowlist via `is_safe_external_url`, dispatches through the `opener` crate) and exposed it on the JS side as `bridge.openExternalUrl(url)`. `sp-footer` now handles the docs/licensing clicks through a `data-action="open-external"` delegate that calls the IPC, so the path no longer depends on the WebView's `with_new_window_req_handler` firing.
- **Footer rendered `v0.7.0 (unknown, unknown)` when `vergen` could not read git state.** `hasBuildMeta` only suppressed the literal string `"unknown"`. Added `isMissing()` to also catch empty values and unreplaced `__PLACEHOLDER__` sentinels, so the parens block disappears when build metadata is missing instead of leaking the fallbacks into the UI.
- **Help & Support section was poorly styled — buttons stretched to the drawer's right border with no breathing room.** Restyled `.sp-activity__help` in `web/css/drawer.css` as a self-contained card: outer margin so it no longer touches the drawer borders, panel background and rounded border for separation, larger gap between title and buttons, and constrained `.sp-btn-ghost` width with left-aligned labels and consistent vertical rhythm.

- **Windows GUI rendered a blank `about:blank` window.** wry 0.55 rewrites custom URI schemes to `http://<scheme>.<host>/...` on Windows/Android because WebView2 cannot register arbitrary schemes, so navigating to `sp://app/index.html` silently failed. Use `http://sp.app/index.html` on those targets and allow the rewritten origin in `allow_navigation`.
- **Native menu bar showed raw i18n keys** (`menu-edit`, `menu-view`, `menu-help`, …). The menu builder calls `i18n::t("menu-*")` but `web/i18n/en-US/bridge.ftl` had no matching entries, so the fallback returned the keys verbatim. Added the seven missing translations.
- **Re-verify button looked broken — actually silent.** Clicking "Re-verify" on a host card fired `host.probe`, ran the probe, applied the snapshot, and emitted `host.changed`, but appended nothing to the activity log. From the user's seat it looked like the click was lost. Added "[host] re-verifying…" before the spawn and "[host] re-verify complete — profile installed, process running" (or equivalent) when the snapshot is applied.
- **Bridge silently continued when the local proxy failed to start.** `gui::run` discarded the result of `proxy::start_default()` and proceeded to render the GUI even when the bind failed, so any profile generated afterwards pointed Claude Desktop / Codex at a dead `127.0.0.1:48217` (`ERR_CONNECTION_REFUSED`). Now: log success/failure to the activity drawer at startup, and refuse profile generation when the proxy isn't listening rather than handing out a profile that can't possibly work.
- **Loopback secret could drift between proxy and host profiles.** Both proxy startup and profile generation called `secret::load_or_mint`, which silently re-minted on a missing key file. If the file disappeared between the proxy's startup read and a later profile-gen read (or vice-versa), the proxy and the host's installed profile ended up with different keys, producing `forbidden: bad loopback secret` (HTTP 403) on every host request. Replaced the dual API with a single source of truth: `proxy_init()` (proxy startup — loads or mints, caches in a process-global `OnceLock`) and `for_profile()` (profile generation and MDM templates — read-only; errors out if the proxy hasn't started). After this change the proxy and any profile generated within the same process can never disagree.
- **Removed broken `__TOKEN__` cache-buster.** Every static asset URL carried `?t=__TOKEN__`, but `assets.rs` substituted `__TOKEN__` with the empty string — leaving meaningless `?t=` query strings that did nothing AND created two distinct module identities whenever one file imported `foo.js?t=` and another imported `foo.js`. Modules ran twice, the second `customElements.define` threw `NotSupportedError`, and the GUI rendered an empty body. Stripped the placeholder from `web/index.html`, `web/css/fonts.css`, all 27 imports in `web/js/index.js`, and the three `.replace("__TOKEN__", "")` call sites in `src/gui/assets.rs`. Embedded assets only change on rebuild, so no cache-buster was ever needed.

### Changed

- **Bridge frontend rewritten off Lit.js — pure vanilla Web Components.** All 22 `sp-*` components migrated from `LitElement` to a 110-line `SpElement` base (`web/js/components/sp-element.js`) with reactive setters, microtask-batched re-render, and `data-action` / `data-input` event delegation. `vendor/lit-all.min.js` deleted. `js/atoms.js` deleted (unused by components — bridge state subscription is the single source of truth).
- **State path unified.** Components subscribe to `bridge.subscribe('state.changed', ...)`, mutate reactive setters, and re-render. `hydrateAtoms` removed from `index.js`. The four parallel communication patterns (bridge sub, atoms, custom events, Lit reactive props) collapse to one.
- **Centralized event registry at `web/js/events/bridge-events.js`** owns all `document.addEventListener` calls (keydown, mkt:count, crumb:set, setup-open). `theme.js` module-scope listeners wrapped in `initTheme()`. Components subscribe via `onBridgeEvent(name, fn)` instead of registering their own document listeners.
- **Oversized components split.** `sp-setup-gateway` 211→117 lines (form rendering extracted to `utils/gateway.js::renderGatewayForm`), `sp-marketplace` 161→138 (listing fetch logic moved to `services/marketplace-service.js`), `sp-cloud-status` 161→127, `sp-rail` 160→119 (tab definitions extracted to `utils/rail-tabs.js`). Every JS file ≤150 lines, every CSS file ≤200.
- **Toast styles tokenised.** Hardcoded hex (`#2a1a1a`, `#d97757`, …) and px (`20px`, `12px`, …) in `main.css` replaced with new `--sp-toast-bg`, `--sp-toast-bg-error`, `--sp-toast-border`, `--sp-toast-fg`, `--sp-toast-shadow`, `--sp-radius-md`, `--sp-z-toast` tokens. Toast block extracted to `web/css/toast.css`.
- **Empty `.catch(() => {})` handlers replaced** with `.catch((e) => console.warn("snapshot failed", e))` across 19 component snapshot calls — visible failure logging instead of silent swallowing.
- **`assets.rs` registry updated** to drop `LIT_VENDOR`, `atoms`, `components/base` and register `components/sp-element`, `events/bridge-events`, `services/marketplace-service`, `utils/rail-tabs`, `utils/gateway`, `css/toast`. The `/assets/js/vendor/lit-all.js` route removed.
- **`i18n.js` leading comment block deleted** (4 lines). `log-virtual.js` switched from `frag.appendChild(li)` to `frag.append(li)`.

### Fixed

- **Clippy cleanup — zero warnings on `x86_64-pc-windows-gnu` and host targets under `-D warnings`.**
  - Removed dead `GuiApp.cancel: CancellationToken` field; cancellation is owned by `AppState.cancels` and per-handler tokens.
  - Collapsed 32 nested `if let` blocks into stable `let_chains` (autofix).
  - Switched four `needless_pass_by_value` sites to borrow: `ipc_runtime::handle_inbound(&str)`, `ipc_runtime::emit_sync_progress(Option<&str>)`, `SettingsWindow::create(&EventLoopProxy, Option<&str>)`.
  - Removed unjustified `#[allow]` attributes:
    - `clippy::unused_self` on `InstallError::exit_code` — replaced with `InstallError::EXIT_CODE` associated constant.
    - `clippy::vec_init_then_push` + `unused_mut` in `integration::registry` — refactored to cfg-gated const slices chained into the registry vec.
  - Audited remaining `#[allow]`s — kept only well-justified FFI (`unsafe_code`), logger-bootstrap fallback (`print_stderr` in `obs.rs`), CLI entry-point output, project-wide stylistic opts in `lib.rs`, `#[cfg(test)]` scopes, and cross-platform signature parity (`unnecessary_wraps` on Linux `org_plugins_system`).

### Added

- **Phase 3 frontend rewrite — full migration from HTTP polling + delegated dispatcher to Lit components + IPC channels.** Every legacy panel under `web/js/` is now an `sp-*` custom element extending `BridgeElement`, hydrated from `state.snapshot` and refreshed by the appropriate channel (`state.changed`, `host.changed`, `proxy.changed`, `proxy.stats`, `sync.progress`, `error`, `log`).
  - **23 new Lit components** in `bin/bridge/web/js/components/`:
    - **Stateless info panels (Phase 3a)**: `sp-proxy-status`, `sp-agent-presence`, `sp-agents-summary`, `sp-overall-badge`, `sp-sync-pill`, `sp-rail-profile`, `sp-footer`, `sp-crumb`.
    - **Interactive panels (Phase 3b)**: `sp-rail` (replaces `tabs.js` + `rail-indicator.js`, owns ⌘1–⌘4 and ⌘F shortcuts, persists `cowork.tab` to `localStorage`, broadcasts `crumb:set`), `sp-toast`, `sp-activity-log`, `sp-host-card`, `sp-hosts-list`, `sp-settings`.
    - **Marketplace + setup wizards (Phase 3c)**: `sp-marketplace`, `sp-marketplace-list`, `sp-marketplace-detail`, `sp-setup`, `sp-setup-gateway`, `sp-setup-agents`.
    - All components use light DOM (`createRenderRoot() { return this; }`) so existing CSS class selectors apply unchanged.
  - **Incremental host updates** — `sp-hosts-list` keeps a `Map<id, host>` and merges per-host deltas from the `host.changed` channel without re-fetching the full snapshot. `sp-agent-presence`, `sp-agents-summary`, and `sp-setup-agents` likewise merge per-host payloads in place.
  - **`bridge.js` shims** added: `openLogFolder`, `diagnosticsExportBundle`, `diagnosticsInfo`. `setup-open` cross-component event lets `sp-settings` reopen the setup wizard.
  - **`crumb:set` `CustomEvent`** decouples breadcrumb updates from `tabs.js`. `mkt:count` `CustomEvent` lets `sp-marketplace` push the marketplace total into `sp-rail` without a shared atom.

### Changed

- **HTTP control-plane server cut to single-instance focus only.** `gui::server::Server` reduced from a full HTTP router (state polling, log polling, marketplace listing, action dispatch, asset serving) to ~85 lines that handle exclusively `POST /api/focus_window` with constant-time CSRF check. The webview already loads via the `sp://app/` custom protocol (`window/native.rs::serve_custom_asset`), so no asset serving needs the HTTP path. Second-launch instances still ping the focus endpoint via `single_instance::ping_focus_running_instance`.
- **`assets::lookup_path` no longer takes a CSRF-token argument** — the `sp://` protocol bypasses it. `__TOKEN__` placeholders in CSS/JS modules are substituted with empty strings.
- **`last_action_message` removed** from `AppStateSnapshot`, `AppStateSnapshotBuilder`, and `StatePayload`. `AppState::set_message` deleted along with all 8 call sites in `handlers/sync.rs` and `handlers/auth.rs`. Toast surfacing now flows exclusively through the `error` IPC channel (`ipc_runtime::emit_error`), which is structured (`{scope, code, message}`) rather than a free-form snapshot field. `sp-toast` simplified to listen to `error` only; `sp-setup-gateway` stops parsing `last_action_message` for failure detection.
- **Marketplace install/uninstall buttons removed** from `sp-marketplace-detail`. Cloud sync (`sync::run_once`) is the install mechanism — signed manifests pulled from the gateway materialize plugins/skills/hooks/agents into `org_plugins_effective()`. Per-item buttons were redundant with sync. Dropped: `marketplace.install` / `marketplace.uninstall` IPC commands, the `MarketplaceItemArgs` struct, and the `bridge.marketplaceInstall` / `marketplaceUninstall` shims.
- **`tabs.js` decoupled from `crumb.js`**: `activateTab` now dispatches `document.dispatchEvent(new CustomEvent("crumb:set", { detail: { name } }))` instead of importing `setCrumb`. (Then both files were deleted entirely as `sp-rail` and `sp-crumb` took over.)
- **`web/index.html` reduced from ~485 to ~120 lines.** Wholesale markup blocks for the rail nav, marketplace tab (categories list + items + detail + actions footer), agents tab, host-card `<template>`, settings panel, activity drawer, setup wizard, and footer all replaced with single `<sp-*>` tags that own their own rendering.
- **`web/js/index.js` reduced from 64 to 43 lines.** No more `applySnapshot`, `subscribePolling`, `subscribeLog`, or `initEvents`/`initKeyboard`/`initTabs`/`initSetup`/`initMarketplace`/`initToast`. Final form: theme + i18n init, side-effect imports for every component, atom hydration from `state.changed`.
- **`gui/command.rs`**: added `openLogFolder` as an alias for `diagnostics.openLogDirectory`.

### Removed

- **Phase 4 — orphaned `http_local` module deleted.** `bin/bridge/src/http_local/` (mod, request, response, hop_by_hop) had zero remaining callers after Phase 3 cut the HTTP control plane. `pub mod http_local;` removed from `lib.rs`.
- **Phase 4 dead-code cleanup**: `Server::csrf_token` field inlined into the listener thread (the cloned token is sufficient); `#[allow(dead_code)]` markers removed from `gui/server.rs::Server` and `gui/menu.rs::MenuBarHandles`. `ErrorScope::Setup` variant dropped — no remaining call sites.
- **18 legacy frontend modules deleted**: `agents.js`, `api.js`, `crumb.js`, `dom.js`, `drawer.js`, `events/keyboard.js`, `events/registry.js`, `footer.js`, `hosts.js`, `hosts/card.js`, `marketplace.js`, `marketplace/detail.js`, `marketplace/glyph.js`, `marketplace/list.js`, `marketplace/state.js`, `overall-badge.js`, `profile.js`, `proxy.js`, `rail-indicator.js`, `setup.js`, `setup/agents.js`, `setup/gateway.js`, `setup/mode.js`, `state.js`, `sync-pill.js`, `tabs.js`. Subdirectories `events/`, `hosts/`, `marketplace/`, `setup/` removed.
- **2 backend Rust modules deleted**: `gui/connection.rs` (HTTP request parsing + CSRF validation + GET routing), `gui/action_dispatch.rs` (POST `/api/<action>` → `UiEvent`).
- **`gui/server_util.rs` trimmed** — `parse_query` and `now_unix` removed; only `mint_csrf_token` and `constant_time_eq` remain.
- **`server_json::snapshot_to_json`** removed (was used only by the deleted HTTP server).
- **`http_local`-based connection handling, `last_action_message` field**, `set_message` setter, builder method `with_last_action_message`, the `last_action_message` payload field, and the `csrf_token` query-parameter validation on asset URLs.

### Notes

- Single-instance focus continues to use a 127.0.0.1 TCP listener (loopback + CSRF). A FIXME in `gui/server.rs` tracks the future migration to Unix domain sockets / Windows named pipes.
- The `sp://app/` custom-protocol asset path remains the only way the webview loads HTML/CSS/JS; the `lit-all.min.js` vendor bundle is served as-is and special-cased to skip `__TOKEN__` substitution.
- `marketplace.list` IPC command and listing payload retained — it surfaces what's already been synced to disk by `sync::run_once`. There is no separate "catalog vs installed" model.
- Single-instance focus across platforms continues to work via the trimmed HTTP server.

### Earlier in this Unreleased window

- **Phase 3 follow-ups (3F.A / 3F.B / 3F.C)**:
  - **Cross-platform menu bar** — `gui::menu::attach_to_window(&MenuBarHandles, &Window)` on Windows extracts the HWND via `raw-window-handle` and calls muda `init_for_hwnd`, attached after settings-window creation. macOS continues to use app-wide `init_for_nsapp`. New direct dep on `raw-window-handle = "0.6"` for the Windows target. Native menu items now go through `i18n::t`.
  - **Cancellation plumbing + UI** — `AppState::install_cancel`/`clear_cancel`/`cancel_scope`/`cancel_all` keyed by a new `CancelScope` enum (`Sync`, `Login`, `GatewayProbe`). `sync`, `login`, `set-gateway`, `logout`, and `gateway_probe` handlers now wrap their `spawn_blocking` futures in `tokio::select!` against a child token; on cancel the result is dropped and a sensible failure outcome is emitted. `on_sync_finished` distinguishes `cancelled` from `failed` and emits a `cancelled` `sync.progress` phase. New `UiEvent::CancelInFlight { scope, reply_to }` + `gui/handlers/cancel.rs`. New IPC command `cancel` (scope `sync` | `login` | `gateway` | `all`) + `bridge.cancel(scope)` JS helper. New Cancel button (`#sync-cancel`) in the sync pill, hidden by default, shown when `sync_in_flight`, wired to `bridge.cancel("sync")`.
  - **Full i18n hydration** — `web/i18n/en-US/bridge.ftl` expanded from ~30 to ~140 keys grouped by surface (setup-, sync-, login-, gateway-, validate-, marketplace-, agents-, status-, settings-, activity-, footer-, nav-, menu-, host-, proxy-). `data-l10n-id` added to every visible static string in `web/index.html`; `web/js/i18n.js` extended to also hydrate `data-l10n-placeholder` and `data-l10n-aria` attributes. JS modules now route every `textContent =` literal through `t()` / `t_args`: `marketplace.js`, `marketplace/detail.js`, `marketplace/glyph.js`, `hosts.js`, `hosts/card.js`, `agents.js`, `proxy.js`, `setup/agents.js`, `setup/gateway.js`, `setup/mode.js`, `sync-pill.js`. Rust handler messages (`auth.rs`, `sync.rs`, `validate.rs`) now use `i18n::t` / `i18n::t_args` for log lines and bridge errors. Translators can drop a `web/i18n/<locale>/bridge.ftl` file and the entire UI switches over.

- In-progress concurrent work staged alongside Phase 2 observability: i18n module + web translation assets, native menu, system process helpers, ipc runtime split, lit-based web components (`atoms`, `bridge`, `theme`, `components/`), tokio-runtime handler refactor (`app.runtime` replacing `app.pool.spawn_task`), proxy/gateway/hosts/integration tweaks. Note: cross-target Windows/macOS build is currently broken in this snapshot pending the GuiApp `runtime` field landing.
- **Phase 2 observability**: support-grade diagnostics surface.
  - Daily log rotation via `tracing-appender` (max 7 files, non-blocking writer).
  - `bridge diagnostics` and `bridge --version` subcommands print version, git SHA, build timestamp, profile, log/config paths.
  - `vergen` build script embeds `VERGEN_GIT_SHA`, `VERGEN_GIT_COMMIT_DATE`, `VERGEN_BUILD_TIMESTAMP`, `VERGEN_GIT_BRANCH`.
  - Footer renders `vX.Y.Z (sha, date)` alongside the version pill.
  - Panic hook writes `bridge-crash-{utc-ts}.log` with payload, location, and backtrace; emits a `tracing::error!` event before abort.
  - Persistent activity log: JSONL writer subscribed to the activity emit hook, atomic byte counter, single rollover at 10 MB to `activity.jsonl.1`.
  - GUI Help & Support drawer panel: "Open log folder" and "Export diagnostic bundle" actions. Bundle zips bridge logs, activity JSONL (+ rolled), crash dumps, redacted config TOML, and `diagnostics.txt`; lands on Desktop and reveals in the OS file manager.
  - HTTP routes `/api/diagnostics/open_log_dir`, `/api/diagnostics/export_bundle`, `/api/focus_window`. IPC commands `diagnostics.openLogDirectory`, `diagnostics.exportBundle`, `diagnostics.info`.
  - INFO-level `gui_dispatch` span with `event_kind` and per-dispatch `request_id` (UUID v4); user-initiated handler entry points promoted from DEBUG → INFO.
  - Single-instance: `bridge.lock.json` sidecar persists `{pid, port, token}`; second launch pings `/api/focus_window` on the running instance (250 ms timeout) instead of silent-exiting.
  - `config::redaction::redacted_config()` walks the loaded TOML and replaces values under sensitive keys (`secret`, `credential`, `auth`, `pat`, `token`, `password`, `key`, `pubkey`, `session`) with `***REDACTED***`.
- New deps: `tracing-appender`, `backtrace`, `opener` (with `reveal`), `zip`, `uuid`, `serde_yaml`. Build dep: `vergen`.

### Changed

- `ActivityLog::set_emit_hook` → `add_emit_hook` (now multi-subscriber `Vec<EmitHook>`); existing IPC subscriber and the new persistent JSONL writer coexist.
- `obs::tracing_init` no longer threads file writes through a static `Mutex<File>`; uses a `NonBlocking` rolling appender behind a `OnceLock<WorkerGuard>`.

- Setup welcome page: drop redundant brand-mark icon from topbar (wordmark only); replace setup-card icon chip with the full systemprompt.io wordmark; hide topbar and footer entirely while in setup mode.
- Primary button (`.sp-btn-primary`) restyled with branded asymmetric corners (`--sp-corners-sm`) and a stable label — removed `transform: scale()` and `translateY` so text size and position no longer shift on hover. Added an icon slot: gray default icon swaps to a rotating spinner via `[aria-busy="true"]`.
- `Connect`, `Finish`, and `Open systemprompt bridge` buttons restructured with `<span class="sp-btn__icon">` + `<span class="sp-btn__label">`. `js/setup/gateway.js` now toggles only the label text on busy, preserving the icon nodes.
- Inputs aligned to `--sp-corners-sm` so form fields share the branded corner profile with buttons and cards.

## [0.7.0] - 2026-04-30

### Added

- `integration::codex_cli` — Codex CLI host integration (probe, config, install).
- `cli::credential_helper` — credential helper command surface.
- `gui::handlers::agents` — GUI handler module for agents.
- `web/css/agents.css` — agent presence cluster, setup-step machine, agents-list-empty, host-card kind chip.
- `web/js/agents.js` — `renderAgentPresence`, `renderAgentsSummary`, `renderAgentsRailCount`.
- `web/js/events/registry.js` — single document-level click registry dispatching `[data-action]`.
- `web/js/events/keyboard.js` — single keydown listener for ⌘1/2/3.
- `web/js/state.js`, `index.js`, `rail-indicator.js`, `crumb.js`, `sync-pill.js`, `profile.js`, `cloud.js`, `proxy.js`, `hosts.js`, `overall-badge.js`, `footer.js`, `marketplace/{detail,glyph,list,state}.js`, `drawer.js`.

### Changed

- **Breaking**: crate renamed from `bin/cowork` to `bin/bridge` (binary name `systemprompt-bridge`). Workspace `exclude` and tests updated.
- `gui::connection`, `gui::dispatch`, `gui::events`, `gui::hosts`, `gui::server_json`, `gui::state`, `gui::mod` — refactored alongside new agents handler and Codex CLI integration.
- GUI assets now serve as 22 modular CSS files and 24 JS ES modules from `/assets/css/*` and `/assets/js/*` instead of inlined into `index.html` via `__STYLE__`/`__SCRIPT__`. Each file is `include_str!`-bundled, served with `?t=<csrf>` token guard, and substituted with the per-request token.
- `web/style.css` (1572 lines, monolithic) split into 22 component files under `web/css/` (`tokens`, `fonts`, `reset`, `kbd`, `dot`, `badge`, `button`, `topbar`, `rail`, `shell`, `drawer`, `marketplace-{base,list,detail}`, `status`, `settings`, `setup`, `agents`, `log`, `footer`, `responsive`, `main`). All custom-property references use the `--sp-*` prefix.
- `web/js/snapshot.js` and `web/js/marketplace.js` (monolithic) replaced by 24 ES modules with named exports only. Single event registry, `data-action` delegation, `<template>` cloning, no `innerHTML` of multi-element strings, no early returns.

### Removed

- `web/style.css` — split into per-component files.
- `web/js/snapshot.js`, `web/js/main.js`, `web/js/activity.js` — carved into the new modules.
- `STYLE` constant, `style_concat()`, `__STYLE__` substitution, and `__SCRIPT__` substitution in `gui::connection`.

## [0.6.0] - 2026-04-30

### Added

- `activity::ActivityLog` ring buffer (1000 entries) capturing live proxy/sync events for the GUI activity feed.
- `proxy::usage` response-stream tap: `is_messages_path`, `wrap_response_stream`. Counts `/v1/messages` calls and sums input/output tokens from JSON and SSE bodies.
- `ProxyStats::messages_total`, `tokens_in_total`, `tokens_out_total` counters.
- `sync::apply::synthetic_plugin` writer: managed skills, agents, and `.mcp.json` are now materialised as a single synthetic Claude plugin (`systemprompt-managed`) under the org plugins root, instead of separate fragments under `.systemprompt-bridge/`.
- `paths::SYNTHETIC_PLUGIN_NAME` constant (`systemprompt-managed`).
- `ApplyError::ReservedPluginId` — manifests containing a plugin with the reserved synthetic-plugin id are rejected.
- GUI: split monolithic `web/app.js` into ES modules under `web/js/` (`main`, `api`, `dom`, `tabs`, `setup`, `marketplace`, `activity`, `snapshot`).
- GUI: `assets/fonts/` bundled fonts and an activity tab driven by the activity log.

### Changed

- **Breaking**: managed assets layout. Skills, agents, and managed MCP servers no longer live under `.systemprompt-bridge/{skills,agents,managed-mcp.json}`; they are written into the synthetic plugin directory `<org-plugins>/systemprompt-managed/{skills,agents,.mcp.json}`. `install` summary, `status`, and GUI counters now read from the new location.
- `install --uninstall` removes the synthetic plugin directory in addition to the metadata directory.
- Plugin sync no longer prunes the synthetic plugin as a stale entry.
- Malformed-plugin counter accepts both `.claude-plugin/plugin.json` and `claude-plugin/plugin.json`, and excludes the synthetic plugin.
- Proxy `forward` now takes `Arc<ProxyStats>` and wraps successful `/v1/messages` responses with the usage tap; counters update on the fly.
- Proxy request handler appends every forwarded request (and client-disconnect / forward errors) to the activity log.

### Removed

- **Breaking**: `paths::MANAGED_MCP_FRAGMENT`, `paths::SKILLS_DIR`, `paths::AGENTS_DIR` constants.
- **Breaking**: `sync::apply::{agent, mcp, skill}` modules. Replaced by `synthetic_plugin`.
- `gui::state::counters::read_index_count` (the old skills/agents `index.json` reader).
- Legacy `bin/cowork/web/app.js`; replaced by ES modules under `web/js/`.

## [0.5.0] - 2026-04-29

### Added

- `auth::ChainError` enum (`NoneSucceeded`, `PreferredTransient { provider, source }`).
- `auth::providers::AuthFailedSource::is_terminal()` distinguishing permanent failures (`PubkeyMissing`, `UnsafePath`, decode errors, `Serialize`) from transient network failures.
- `auth::evaluate_chain()` — chain evaluator accepting an explicit provider list and preferred-provider hint.
- Exit code `10` on `cli run` and `cli whoami` for a transient failure on the configured preferred provider (distinct from `5` for "no credential source succeeded").

### Changed

- **Breaking**: `auth::acquire_bearer` and `auth::mint_fresh` return `Result<HelperOutput, ChainError>` (previously `Option<HelperOutput>`).
- **Breaking**: `UiEvent::{SyncFinished, LoginFinished, LogoutFinished, SetGatewayFinished}` and `HostUiEvent::{ProfileGenerateFinished, ProfileInstallFinished}` payloads now carry `Arc<GuiError>` instead of `GuiError`.
- **Breaking**: `gateway::GatewayClient` request timeout reduced from 30 s to 10 s.
- Preferred mtls provider with a transient gateway failure no longer silently falls through to PAT.

### Removed

- **Breaking**: `GuiError::Msg` variant and the manual `Clone` impl on `GuiError`.
- **Breaking**: `http_local::request::parse(&mut TcpStream)`. Use `parse_from_read` (any `Read`) or `parse_buffered` (any `BufRead`).
- All inline (`//`) and doc (`///`) comments under `bin/cowork/src/`.
- Unused `CODE_DOMAIN` constant in `integration::claude_desktop::shared`.

### Fixed

- Proxy dropped HTTP/1.1 trailers as silent empty data frames; non-data frames are now filtered out before the upstream body is forwarded.
- Proxy `io::Error` boundary preserves the source chain instead of stringifying via `to_string()`.
- Tokio runtime initialiser returns `io::Error` on the `OnceLock` race instead of `process::abort`.
- Proxy listener binds IPv4 loopback (`127.0.0.1`) first and falls back to IPv6 loopback (`::1`); previously bound dual-stack `[::]:port`, exposing the proxy to non-loopback peers on hosts where `IPV6_V6ONLY` was off.
- Windows Claude Desktop profile generator emits `inferenceModels` as `REG_MULTI_SZ` (`hex(7):`-encoded UTF-16LE) instead of a comma-joined `REG_SZ`.
- `auth::cache::write` and `proxy::secret::load_or_mint` log a `tracing::warn!` when `chmod 0600` fails on the cached file, instead of swallowing the error.

## [0.4.0] - 2026-04-27

### Added

- Native GUI on Windows and macOS; `gui` subcommand launches a branded settings window (gateway URL, PAT input, cached-JWT state, marketplace counters, plugins-directory path, last-sync timestamp, activity log).
- Default routing falls through to `gui` when launched without an attached terminal; terminal invocations continue to emit the JWT envelope to stdout.
- Tray menu items: Sync now, Validate, Open settings, Open config folder, Quit.
- `sync::run_once` returns a structured `SyncSummary` / `SyncError`; `validate::run` returns a structured `ValidationReport`.

### Changed

- Linux `gui` exits `64` with `gui not supported on this platform`.

## [0.3.3] - 2026-04-23

### Changed

- Release-only bump; no code changes vs 0.3.2.

## [0.3.2] - 2026-04-23

### Added

- `install --apply` on macOS direct-writes `/Library/Managed Preferences/com.anthropic.claudefordesktop.plist` and restarts `cfprefsd` (single sudo prompt, no MDM required).
- `install --apply-mobileconfig` builds a `.mobileconfig` and opens System Settings → Profiles for approval (MDM workflow).
- `uninstall` removes both managed-prefs plists and kicks `cfprefsd`.

### Removed

- `profiles install` / `profiles remove` invocations (deprecated by Apple on macOS 11+).

### Fixed

- Reject `http://` for non-loopback gateways at install time.

## [0.3.1] - 2026-04-23

### Notes

- Superseded by 0.3.2; did not ship.

## [0.3.0] - 2026-04-22

### Added

- `whoami` subcommand prints authenticated identity from the gateway.
- `sync` materialises `user.json`, `skills/<id>/{metadata.json, SKILL.md}`, `agents/<name>.json` under `.systemprompt-bridge/`.
- `status` surfaces identity and skill/agent counts from on-disk fragments.

### Changed

- **Breaking**: signed-manifest wire format extended with `user`, `skills`, `agents`. `AgentEntry.card: object` replaced with `system_prompt: string?`. 0.2.x clients cannot deserialise 0.3.x manifests.
- Manifest signing primitive moved to `systemprompt-security::manifest_signing` (signature semantics unchanged).
- Per-user manifest assembly relocated from the gateway into the template admin extension.

## [0.2.0] - 2026-04-22

### Added

- `ed25519-dalek` dependency for signed-manifest verification.
- Plugin / MCP sync against Cowork's `org-plugins/` mount.

### Changed

- **Breaking**: crate renamed to `systemprompt-bridge` (binary `systemprompt-bridge`, lib `systemprompt_bridge`).
- Manual release via `cargo-zigbuild` + `gh release create` on tag `cowork-v*` (Linux x86_64 + Windows x86_64 binaries).

## [0.1.0] - unreleased

### Added

- Initial scaffold: JSON wire contract, cache, blocking HTTP client, platform keystore trait (macOS/Windows/Linux stubs), SSO assertion fetch, stdout JSON emission.
