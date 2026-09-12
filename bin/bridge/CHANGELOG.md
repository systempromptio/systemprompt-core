# Changelog

## [0.52.0] - 2026-09-12

### Changed

- The tray's update action follows the current state: it checks, installs an available release, reports progress, or restarts into a staged release. The tray and bottom account menu also expose confirmed flows to disconnect this computer, reset all Bridge state, or clean up and reveal the application for platform-managed removal.
- The README's runtime-environment table is the complete environment contract: `<PREFIX>_POLICY_TRUST`, `<PREFIX>_EGRESS_ALLOWED_HOSTS`, the per-platform `<PREFIX>_DEVICE_CERT` / `_DEVICE_CERT_LABEL` / `_DEVICE_CERT_SHA256` readers and `RUST_LOG` are listed alongside the variables it already named, and the OS directory/identity probes and the Codex/Hermes home overrides are enumerated. Every environment read in the bridge is allowlisted with its reason under `scripts/env-var-allowlist.txt` (`just lint-env-vars`); no reader was found that switches behaviour outside that table.

## [0.51.0] - 2026-09-11

### Added

- `ManagedMcpServer.tool_policy` (a `*` entry plus per-tool overrides) is projected to each host: Claude Code receives `permissions.allow`/`deny` rules (`mcp__<server>`, `mcp__<server>__<tool>`, and the `mcp__plugin_<plugin>_<server>` spellings for plugins whose `.mcp.json` mirrors the server) in the managed settings file when writable and `~/.claude/settings.json` otherwise, recorded in a sidecar so a server that leaves the manifest has its rules withdrawn; Claude Desktop receives `managedMcpServers[].toolPolicy` for every tool the server reported to the auth probe, kept in `metadata/mcp-tools.json`. A person's own rules are never touched.
- A host sync can warn without failing: `SyncSummary.host_warnings`, the sync line's `— N warning(s)` suffix, and a warning row on the Status page and in the activity log. The Cowork emitter names the missing step when Claude Desktop is installed but Cowork has never been opened, and the desktop probe triggers the sync once the session directory appears.
- `HostApp::install_profile` returns `ProfileInstalled { warnings }`; `install --host` and the reapply report print each warning under the host's line.
- **Windows:** the Cowork session root is also looked for under the MSIX package's `LocalCache\Local\Claude-3p`.

### Changed

- Tools on managed MCP servers are allowed by default: a server whose YAML sets no `tool_policy` carries `*: allow`; `prompt` and `deny` are opt-in per server or per tool.
- A wildcard `deny` withholds the server from Claude Desktop's managed list instead of expanding over the tool catalog, so a tool the catalog has not seen cannot fall back to asking; under a wildcard `deny`, named `allow` rules are not written for Claude Code, whose deny list would override them anyway.
- The tool catalog (`metadata/mcp-tools.json`) is read fallibly: an absent file is empty, a corrupt or unreadable one is an error that fails the policy write and is never rebuilt from empty; a catalog write that fails after a probe is reported as a host warning (sync) or an activity-log warning (GUI).
- `HostWarning` lives in `host_sync` (re-exported from `sync`), beside the `HostWarnings` collector.

### Fixed

- **Windows:** running the bridge already elevated failed the Claude Desktop profile install with `os error 1346` after provisioning had succeeded; the Modify check now passes the linked token to `AccessCheck` as-is. A Modify check that could not run is a warning on a successful install; a check that ran and found the unelevated user without Modify access fails the install.
- **Doctor:** the stale hook-port check (mirrored `hooks/hooks.json` naming a port the proxy no longer holds) existed but was never run.

## [0.50.0] - 2026-09-10

### Added

- `doctor` gains a `private files` check that opens the config, PAT, loopback key, install id and port file as the current user and prints the owner and DACL of any it cannot.
- The diagnostics bundle and `diagnostics` carry `registry.txt`: both hives of `SOFTWARE\Policies\Claude` and the bridge policy key, value by value with secrets as fingerprints, each key's owner, DACL and last write, the process token's elevation and SID, and the WebView2 runtime version. `state.txt` gains the org-plugins root with each plugin directory's ACL, every host profile's state, keys and file ACL, the staging and metadata directories, the single-instance lock and the update policy. `bridge-proxy.json`, `bridge-install.id` and `last-sync.json` are included verbatim.
- Rules are a marketplace category: `build_listing` reads each plugin's `rules/` directory, deduplicates by id, annotates the entries with the last sync diff and merges the gateway's `Rules` category, and the GUI lists them between skills and agents. `SignedManifestBuilder::with_rules` carries `RuleEntry` records, the sync summary line reports a rule count taken from the bundled `rules/*.md` files, and `last-sync.json` records `rule_count`.

### Changed

- The `install identity` startup fault carries the same path and remedy as the loopback secret fault.

### Fixed

- **Windows:** the private config directory grants inheritable access to the user, SYSTEM and Administrators. Protecting it with non-inheritable entries left files written by an earlier release (`bridge-loopback.key`, `bridge-install.id`) with an empty DACL that denied even their owner, so the proxy failed before binding and every host sync failed at "read loopback secret". The first start of this version restores access to those files and keeps the existing secret, so installed host profiles stay valid without a re-apply.
- **Windows:** a private file this user owns but cannot read is repaired in place on read and logged at WARN with its descriptor before and after; a file owned by another account is reported as such, with the reset remedy.
- **Windows:** a plugin directory under the system org-plugins root that the current user cannot replace triggers the one-time elevated Modify re-grant on the tree, and the manifest is applied again; a second failure names the directory and the `icacls` remedy instead of `remove old <plugin>: Access is denied`.

## [0.49.0] - 2026-09-09

### Breaking

- **Breaking:** the `[update] automatic` config key and `config::UpdateConfig` are removed. Whether a bridge updates itself is `bridge_policy.auto_update` on the gateway, delivered on the signed manifest; `update::automatic_enabled` reads that. Migrate by setting the policy on the gateway and deleting the local key.

### Added

- The bridge checks for a newer release every six hours and after waking from sleep, and — when instance policy says `staged`, the default — downloads it, verifies its digest and swaps it on disk without restarting. The next launch runs the new version. Until now the only periodic check ran in the settings window's JavaScript, so a bridge sitting in the tray never checked at all and stayed on its installed version indefinitely.

### Changed

- A pre-0.48 `[sync] pinned_pubkey` is adopted as operator trust for the configured gateway (and dropped, like any operator pin, when it recorded another), and the first sync that verifies against it rewrites it as `[sync.trust]`. 0.48.0 reported it stale and blocked every sync with a remedy only an administrator could run; an install that had synced the day before was left with no in-app way back.
- The loopback secret is read before a port is bound. A key file the OS refuses to read used to surface as `Tried ports []: Access is denied` with no path and no remedy; it is now a named startup fault that carries the file and the fix, and the Status page offers **Reset local proxy secret**, which re-mints the key and restarts the bridge.
- A default port held by another bridge install (another Windows account running the bridge) is a startup fault naming that install's config directory, not a log line. Every profile written for that port authenticates against the other bridge, which is why the hosts reported "proxy timed out" with no visible cause.
- Repairing Claude Desktop on Windows requests administrator approval when `HKLM\SOFTWARE\Policies\Claude` already holds other values, instead of failing with "already holds different values" after announcing a UAC prompt that never came. An elevated sync that replaces a machine policy carrying another bridge's secret says so by fingerprint.
- **Remove everything** removes everything: the loopback key, install identity and port record go (a kept key kept its fault and the next start failed the same way), every enrolled host's profile is cleared rather than only Cowork and the Claude Code CLI, the whole machine Claude policy is cleared through an elevated write, and whatever could not be removed — including a bridge running under another account — is reported rather than left behind.
- `diagnostics` and the diagnostic bundle carry `state.txt`: proxy role, who answers the default port, the port file, the config directory with per-file readability and (Windows) owner + DACL, the effective Claude Desktop policy, and visible bridge processes. The bridge logs its version and commit on every start.

### Fixed

- Credential refresh defers transport and gateway 5xx failures until the next tick. Explicit 401/403 responses and unreadable local credentials require sign-in. Proxy errors distinguish service unavailability from authentication rejection.
- "Session expires soon — sign in again" no longer fires seconds after every sign-in. It was raised on the short-lived access JWT, which a stored PAT renews unattended; it now only warns when there is nothing left to renew from.

- **Restart to finish updating** no longer races its own successor. The proxy stops accepting and drains in-flight requests before the new process is spawned, and the new process waits for the old one to release the single-instance lock. Previously the successor could lose the lock race and merely focus the dying window, or bind a fallback port that every host profile written for 48217 rejects.

- Marketplace skills and counts remain visible during gateway probes and temporary outages. Signing back in reloads an unchanged manifest, stale listing replies cannot overwrite a newer session, and failed refreshes retain the previous list with a retry action.
- `alert_user` no longer holds its caller until the dialog is dismissed. The macOS `osascript` dialog and the Windows `MessageBoxW` were both modal and blocking, so an installer path that raised one on an unattended host, or the native test job on a CI runner, waited forever; the dialog is now raised and reaped on its own thread. The Quality workflow's native bridge job also carries a 45-minute timeout.

## [0.48.0] - 2026-09-08

### Breaking

- **Breaking:** manifest signing trust is a `[sync.trust]` record (`gateway`, `key`, `source`) bound to the normalized gateway identity, and the managed-policy key is `manifestTrust` carrying the same JSON record. Operator trust is per gateway: pointing the bridge at another gateway trusts on first use again, while a managed pin for another gateway is refused as stale. A legacy `[sync] pinned_pubkey` is reported stale and blocks sync; a bare policy `manifestPubkey` is adopted for the configured gateway only when nothing else pins. Migrate by re-pinning the key for the intended gateway with `install --apply --pubkey <base64>` or `sync --allow-tofu`.
- **Breaking:** `config::load` and `Config::load` return `Result<Config, ConfigReadError>`; an unreadable or malformed config file is an error rather than defaults. Migrate by handling the error at the call site.
- **Breaking:** `config::write::{set, set_if_absent, remove}` return `Result`, and `edit`/`edit_file` take a closure returning `Result`. An edit through a scalar parent is `ConfigWriteError::InvalidPath`; a file changed by another writer during the edit is `ConfigWriteError::ConcurrentEdit` and is left untouched.
- **Breaking:** `auth::cache::write` is replaced by `write_bound`, and the cache readers return `io::Result<Option<_>>`; a malformed cache or one written under another credential binding is an error, not a miss. `TokenCache`'s refresh closure resolves to `Result<HelperOutput, ForwardError>` and the stamp-check interval is gone. Migrate by capturing a `CredentialBinding` before the exchange.
- **Breaking:** `reload_runtime_config`, `RuntimeConfig::from_loaded` and `shared_from_loaded` return `Result`; `portfile::read` returns `io::Result<Option<PortRecord>>`. `ProxyHandle::{serve, attach}` take a `&mut Vec<StartupFault>` and are infallible: a port file, config or registry fault is recorded rather than returned.
- **Breaking:** `BridgeContext` carries `startup_faults: Vec<StartupFault>`, `GatewayProbeOutcome` carries `credential_error`, `MarketplaceItem` carries `error` and `MarketplaceListing` carries `last_sync_error`; `InstallError::ScheduleActivation { units, reason }` reports scheduler units written but not activated.
- **Breaking:** `ConfigStore::{policy_key_exists, read_policy_document, write_policy_values, delete_policy_values}` and `verified::{apply, remove_values}` take a `PolicyTarget` (`Claude` or `Bridge`); `write_bridge_policy` is gone, the bridge signing-trust key is written by the same verified algorithm as Claude's policy. `auth::cache::{read_valid, read_with_threshold}` are gone in favour of `read_for(&cfg, …)`, and `write_bound` takes the caller's `Config`. `SyncError` gains `Authentication(ChainError)`, `Provision(io::Error)` and `Elevation(String)`; `ChainError::exit_report()` is the one exit-code and message table for `run`, `whoami`, `update`, `credential-helper` and `doctor`. `InstallSummary.completed`, `CredentialsOutcome::PurgeFailed` and `MdmError::ApplyElevation` are removed; `tasks` and `windows_acl` are crate-private.
- **Breaking:** `HostSyncCtx` carries a `policy_store`, `InstallSummary` carries the completed `InstallStep`s, and `MdmDisplay::Applied` holds an `MdmApplication` report of verified files and policy receipts. `MdmDisplay::MobileconfigApplied` is `MobileconfigPrepared`, since a profile is only installed once the user approves it in System Settings.
- **Breaking:** `validate::run` takes only the HTTP client; the in-memory "TOFU key not persisted" flag is gone because a key that cannot be persisted now fails the sync.

### Changed

- **Breaking:** the bridge crate version tracks the core release. It had its own numbering (0.38.0) while the white-label brand crate carried a third (0.1.1), so the footer, the heartbeat and the admin Devices page each showed a different number. The wire version stays `brand::COMPAT_VERSION`; `warn_if_version_drifts` logs once at start-up and `doctor` reports a `bridge version` check when a brand build displays a number it does not report.
- Trust-on-first-use persists the fetched key only after the manifest verified and decoded against it, and a persistence failure fails the sync instead of leaving the next run to trust whatever key the gateway serves. `doctor` and `validate` name the gateway a pin belongs to.
- **Windows:** unelevated policy writes target HKCU; elevated writes target HKLM. Admin-owned files and org-plugin paths require elevated provisioning. Conflicting HKLM values return `HiveConflict`; identical values satisfy the write plan.
- Claude Code plugin skills are filtered by target host: a skill whose manifest entry names hosts that do not include `claude-code` is removed from the mirrored plugin tree. A skill with no host list is a legacy entry and is kept for every host.
- Sync derives non-Claude `modelPicker` entries from provider health and writes owned settings fragments and already-merged user settings. A sidecar tracks owned rows for removal. Claude model entries remain discovery-managed.
- The Claude Desktop policy's `inferenceModels` is filtered to Claude ids where the key is built, falling back to the default list. Desktop breaks on non-Anthropic families.
- A plugin's files are fetched with bounded concurrency (8, matching the HTTP pool) instead of one at a time. Staging is unchanged: the plugin becomes live only after every file succeeds.
- The bridge profile locale gains the connected-accounts section key.
- The Windows policy write goes through the FFI store rather than `reg.exe`, so the loopback secret no longer appears on a command line.
- Managed-policy writes go through `config::store::verified::apply` over an injectable `ConfigStore`: the target hive is read exactly, an unelevated write that an existing HKLM key would shadow is `HiveConflict`, identical values are `PolicyWrite::AlreadyVerified`, and every write and deletion is read back. `PolicyStore` is owned by the context and passed through install and host sync.
- Atomic file replacement stages an exclusively created temporary file, syncs it, reads the destination back and checks Unix permissions; Windows private files (PAT, config, cache, loopback secret) are created with a protected DACL for the requesting user, SYSTEM and Administrators, and the descriptor is verified before and after the write.
- The elevated Windows job carries a protocol version, a job id and the requested steps, and the child writes a `started` result before any work. The parent accepts only a matching, completed result with a zero exit and verifies installed and removed files afterwards.
- A sync whose apply reports host failures or malformed plugins returns `SyncError::Partial` with the structured summary and does not advance the replay checkpoint. Replay-state, token-cache, workspace, scheduler and activity-log persistence failures reach the caller.
- Background GUI and proxy tasks are owned by a `TaskOwner` that is cancelled on drop and reports panics; stale mount and peer replies are rejected, and UI state comparison uses explicit semantic fields rather than a filtered hash.
- Uninstall fails on a staging, plugin-tree, scheduler or credential-purge cleanup it could not complete rather than warning and reporting success.
- `sync` with no credential exits 5 (`NoCredential`) and an authentication failure carries the chain error, instead of both collapsing into the network error (exit 3); a filesystem or elevation failure during org-plugins provisioning is `Provision`/`Elevation`, not `Network`.
- A signing-trust record for another gateway is judged stale (policy) or unpinned (operator) before its key is decoded, so a stale record with a malformed key no longer surfaces as a key-encoding error.
- Temporary probe and staging clean-ups log a warning when they fail instead of discarding the result; the macOS bridge preference templates name `manifestTrust` directly.
- `just lint-discarded-results` and `just lint-fail-open` parse the bridge sources and reject a discarded `Result` without a `// Why: discard-ok:` justification and a guard that returns `true` on the wildcard arm; both run in Quality CI.

### Fixed

- Start-up no longer bricks the tools that repair it. A corrupt or unreadable proxy port file, MCP registry cache, activity log, log directory or config file is recorded as a `StartupFault` on the context, reported by `doctor` as `[FAIL] startup` and listed under health in the GUI, while `--version`, `whoami`, `login`, `install --apply` and `doctor` itself still run. A placeholder install id is re-minted; an unwritable one is replaced by a process-only id for this run.
- The proxy keeps serving when its port file cannot be written; the unpublished port is a start-up fault `doctor` reports. In attach mode a recorded port held by an unidentified listener falls back to the default port with a fault instead of refusing to start.
- The gateway probe reports the gateway's own status. A credential that cannot be minted, a cache that cannot be cleared or a bridge profile that cannot be fetched is `credential_error` beside a `Reachable` status, rather than `Unreachable`.
- A credential cache that cannot be parsed is removed and treated as a miss so the next mint replaces it; only a removal that fails is an error. The proxy's token cache checks the credential stamp at most once every five seconds, off the cache mutex, instead of reading the config and keystore on every forwarded request; an unreadable stamp discards the cached token rather than failing the request.
- **macOS:** `install --apply` writes Claude Code gateway settings and default models through the Unix settings writer. Machine policy paths use `claude_code_policy_dir`; `apiKeyHelper` paths containing whitespace are shell-quoted.
- `install --apply` writes the standalone `claude-code-settings.json` fragment and leaves `~/.claude/settings.json` alone; merging the gateway keys into the user's own settings is the `--host claude-code` enrolment's job, and `uninstall --host claude-code` removes exactly those keys. Every apply used to rewrite the user's settings, which made the per-session `claude --settings` toggle impossible and surprised developers keeping a personal Anthropic login.
- Sync persists derived signing pins as operator trust in the config file. Administrator policy trust is written through `install --apply --pubkey`.
- Setup preserves expanded details across snapshot updates, identifies the selected gateway and whether it is a brand default, and clears credential-rejection notices when a verified identity is available.
- A `marketplace.list` reply that never lands rejects into the pane's error card with a retry button instead of leaving the pane on its skeleton for the life of the window. Inter-process calls gain an opt-in reply deadline, which `marketplace.list` sets to 30 seconds; long-running commands such as sync and host installs under UAC keep no deadline. The listing fetcher also refetches when a sync has just finished even if a request is still marked in flight.
- The log tee writes the file leg when stderr is closed and vice versa, and reports the first failure after both ran.
- On macOS, `killall cfprefsd` exiting 1 with "No matching processes" completes the managed-preferences write; the daemon was not running.
- Bootstrap restores ownership with `chown -R`, so the tree root created is not left with root-owned children, and verifies a sampled child as well as the root.
- Failures after a side effect landed are reported as partial with receipts: on Windows an org-plugins check that fails after the policy write names the hive the policy is in; on Linux scheduler units written under `~/.config/systemd/user` but not activated (no user manager in a container or WSL) are `InstallError::ScheduleActivation` listing the files.
- Cancelling a login interrupts the running login task, not only one that has not started.
- The marketplace lists an item whose skill file, manifest, README or child directory cannot be read, with the error on that item, and reports a corrupt last-sync sentinel on the listing; one broken plugin no longer hides the rest.
- Exporting a diagnostics bundle replies with the bundle path; a file manager that cannot reveal it is a log line, not a failed export.
- `uninstall` on macOS skips the administrator prompt when no managed preferences exist and the unprivileged profile inventory does not list the bridge profile.
- A bridge with no credential configured reports `ChainError::NoneSucceeded` and the `login` hint from `doctor`, `update` and `run` (exit 5) rather than a credential-cache error; the credential binding is captured only once a credential source exists.
- The last-sync checkpoint is written as soon as the manifest is applied, before the default model is seeded from the bridge profile, and a gateway without the profile endpoint (404) no longer fails the sync.
- Registry writes are read back and compared, returning `VerifyMismatch` with hive, subkey and value name. Drift checks read the target hive. Elevated operations require a parseable result file; deletion failures are logged.
- The sync toast and log carry the first line of each host failure beside the host id, so a registry failure reads as "claude-desktop: HKCU\…\Claude value inferenceGatewayBaseUrl did not land" rather than "claude-desktop".
- A signature failure names where the pin came from (config file or policy), which gateway served the manifest, and the fix for that source. The old text blamed tampering for what was almost always a pin for a different gateway.
- `allowedWorkspaceFolders` includes the home directory after the default brand workspace. `doctor` reports effective roots and warns when home is absent.
- `doctor` gains a `claude policy hive` check on Windows: which hive holds the policy, FAIL when HKLM shadows a differing HKCU copy, WARN when elevated with only an HKCU policy.
- The Library pane no longer repaints on every state snapshot after a failed sync. The refetch marker was keyed on `state == "ok"`, so an `error` state refetched — and painted `loading` over an existing listing — on each 30 s host probe.

### Removed

- The `test_api` modules; login, proxy, update, managed-file, Claude Code managed-settings and sync-apply helpers are public in their own modules.

## [0.38.0] - 2026-09-06

### Changed

- **Operator-visible:** Claude Code mirrors each gateway marketplace into its own marketplace and cache directories with `<plugin>@<marketplace-id>` registry keys. A sidecar tracks owned marketplaces for pruning; user-managed marketplaces are preserved. The first marketplace-aware sync replaces the legacy layout. Empty marketplace lists retain `org-provisioned` compatibility. Doctor checks all owned marketplaces and hook URLs.
- The managed-skills sidecar (`.systemprompt-managed.json`) that OpenCode and Hermes share records the ids of the marketplaces the manifest listed under `marketplaces`, so the grouping the flat skills directory cannot show is still readable on disk.
- Exposed non-terminal bridge helpers to the separate test workspace through hidden `test_api` modules for proxy, update, login, managed files, Linux settings and sync. Production behavior is unchanged in this release.
- The `accepted_surfaces` rationale moved into the module head, where it belongs: it describes when the gateway can serve a provider over a wire the provider does not speak natively, which is a property of the module rather than of the one function it sat above.

### Fixed

- **Operator-visible:** OpenCode advertises gateway models across supported Anthropic, OpenAI and Gemini surfaces. Backend-only providers remain excluded. Other host advertisement filters are unchanged in this release.
- The OpenCode module head claimed the managed tier meant no user config could route inference around the gateway. It does not: the file is mode 0644, Linux falls back to the user tier when `/etc` is unwritable, and nothing stops a user adding another provider. It now says tier preference, and points at the gateway as where governance is actually enforced.
- `cmd_login` no longer carries a usage arm it could never enter. `code` was `None` only where a pasted token was `Some`, so the exit-64 branch was unreachable; the control flow expresses that invariant directly and `usage()` is gone.
- `finish_login` took its token and gateway by value without consuming them, and the Linux settings module head opened with a paragraph `clippy::too_long_first_doc_paragraph` rejects. Both cleared, so the Quality gate passes.

## [0.37.0] - 2026-09-04

### Added

- `install --host <id>` accepts repeated or comma-separated IDs; `--hosts all` selects the registry. Unknown or suppressed hosts fail the request, sync-only hosts report no local installation, and manifest-disabled hosts are skipped with a reason. Enrollment shares re-apply inputs. Proxy guidance includes enrollment commands and an OpenCode provider snippet.
- `uninstall --host <id>` un-enrols one host, removing just that host's bridge-owned settings and leaving the bridge installed. A bare `uninstall` is unchanged. Both paths share the resolution and reporting of `install --host`. The only way to undo an enrolment had been the GUI's Remove button, which a headless Linux box does not have — the docs could describe enrolment and not its reverse.
- OpenCode on Linux falls back to the user tier (`~/.config/opencode/opencode.json`) when the managed tier is not writable, because there is no elevation to offer there. `probe` reads that tier so the host is not reported as unconfigured, and `uninstall` sweeps both. macOS and Windows are unchanged: they can prompt, so a refused managed write stays a refusal.

### Fixed

- Browser sign-in tries `LOOPBACK_PORTS`, starting with 8767 and skipping only addresses already in use. Callback URLs use the bound port. Exhaustion reports the attempted ports; other bind errors identify their port.
- A sign-in failure that never reached the log file. The bind error was mapped straight into `AuthError` and rendered as a GUI toast, so a wedged sign-in left no trace in `bridge.<date>.log` — the only artefact available when nobody is at the machine. `capture_device_link_code` now diagnoses it the way the neighbouring browser-launch failure already did.
- Sign-out and device purge cancel in-flight login before clearing credentials. Cancellation closes the callback listener and prevents login completion from restoring removed credentials.

- Validation runs after sync completion on success or failure. Checks during provisioning report a warning while the requested configuration is still being applied.
- Trust-on-first-use persistence failures return configuration errors and record their cause in the activity log. Validation distinguishes fetched-but-unpersisted trust from missing provisioning.
- macOS managed preferences support JSON serialization of property-list values, including arrays. Plain strings retain their existing representation.
- Claude Code policy validation uses the removal predicate: `managed-mcp.json` is identified by presence, and `managed-settings.json` only when it contains bridge-owned keys.
- A white-label build printed the hardcoded systemprompt host as the MDM snippet's fallback gateway, which an admin could paste verbatim. It falls back to the brand's own `default_gateway_url`, and the README stops naming that host as the documented default.
- `docs_url` was documented as a base with the platform appended, but both the setup footer and the main footer link it verbatim, so the value had to be a page that exists on its own. It points at `/documentation`.
- `render_index_from` substituted `__CORE_VERSION__` with `COMPAT_VERSION`, which is the bridge's own compatibility version and not core's. The placeholder is renamed `__BRIDGE_VERSION__` to match the value it carries.

### Changed

- The session-provider test no longer asserts that a candidate loopback port is simply free. That made it depend on whatever else was running on the machine and it failed wherever Docker Desktop held 8767, for a reason unrelated to the provider. It compares the occupied candidate set before and after instead, which is what "the provider starts no listener" actually means.

## [0.36.0] - 2026-09-03

### Added

- Added device purge under the sign-in form with a confirmation step. `purge_device` removes credentials, configuration, onboarding state, managed profiles, plugins and schedules, then reloads the first-launch state.

### Fixed

- The module layering gate passes again: `install::mdm` named `integration::claude_desktop` for the default model list, and `validate::policy` named `install` for the legacy pubkey key. The model list now lives in `install::mdm` beside the gateway block that writes it and the Claude Desktop host reads it from there, and the legacy key sits in `config::store` beside the key that replaced it — both below every module that names them.
- macOS `cargo clippy -D warnings` builds again: `build_bridge_prefs_plist`'s `{pubkey}` placeholder tripped `literal_string_with_formatting_args` while the `#[expect]` sat on `build_prefs_plist`, whose placeholders all carry an underscore and never fire it. The attribute moved to the function that needs it.
- Cowork artifact sync stages the dashboard bundle in the trusted workspace independently of session-directory creation. Empty artifact sets preserve the staged bundle.
- macOS sync no longer fails closed on a missing `/Library/Application Support/Claude/org-plugins`. That early return also skipped the MCP registry publish and every policy write behind it, so one absent directory zeroed the managed MCP registry and told a double-click user to go and run `sudo … install --apply`. It raises the same one-time administrator prompt Windows already used for the equivalent case, and hands the directory to the invoking user so later unelevated syncs can write.
- macOS managed MCP servers can authenticate. The block published the upstream gateway URL with no headers, where Windows publishes the loopback proxy URL plus the bearer, so those servers could never authenticate and any request that did leave carried no per-user identity and bypassed governance. macOS now matches the Windows shape, and `oauth` goes with it — an empty dict asks for well-known discovery against a URL that authenticates by bearer.
- The sync emitter's macOS arm was a no-op that reported "written by install --apply", so a manifest that gained or lost a server never reached the plist and connectors did not sync on macOS at all. It re-renders the managed preferences now, as Windows re-asserts the registry.
- Entropy scanning handles filesystem paths per segment, exempting whole path-shaped tokens while retaining detection of credential material within segments.
- `integration::resolve_host` classifies IDs as `Local`, `SyncOnly`, `Suppressed` or `Unknown`. GUI handlers use one resolution boundary; suppressed hosts report installation-specific unavailability.
- The Windows elevation job file was one fixed name, so the first-run host-profile write and the first sync's org-plugins provisioning overwrote each other: the elevated child ran the org-plugins job twice, both callers read the same `ok` result, and the profile was logged as installed with no registry write behind it. Every elevation request now stages its own job and result file and removes them afterwards.
- Installation and sync write the complete gateway policy block: provider, loopback URL, credential, auth scheme and models. Unreadable credentials fail sync. Validation requires the provider, URL and key together.

### Changed

- The sync summary line counts artifacts alongside plugins, skills, agents, hooks and MCP servers, so a sync that staged no dashboards is visible in the line itself rather than only in the workspace folder.
- The Claude Desktop install label says the profile is *offered*, not loaded: on macOS `open -g` only queues it for System Settings, so the probe a second later correctly reported "profile not installed" and the pair read as a contradiction. The install path now states the approval steps the way the Windows path states its UAC prompt.
- The setup footer names the code, not just the brand. A white-label build pins its own display version (Astound ships `0.1.1`), which says nothing about the bridge underneath, so a screenshot of the wizard could not identify the build; the core version now sits beside it and the commit is on the title attribute.
- The update affordance is reachable from the setup wizard. It lived only in the signed-in rail, so a user who never got past onboarding was never told a newer build existed; the wizard checks once sign-in lands (the endpoint is authenticated, so it cannot run earlier) and offers install/restart in place.
- The dev fixtures carry every `KNOWN_HOSTS` id — `no-models.json` and `proxy-down.json` had quietly dropped `codex-cli`, and nothing checked — and `mock-ipc` rejects an unknown `host.probe` id the way the real handler does, so the preview can show the failure it used to hide.
- Manifest trust uses the brand’s own Windows policy key or macOS managed-preference domain. Sync removes `inferenceManifestPubkey` from Claude policy, validation reports remaining copies and MDM snippets use the brand location.
- Rail order is Marketplace, Agents, Account, Settings, Status, Activity (Ctrl/⌘ 1–6 follow); the Activity pane header lost its leftover column divider.

### Removed

- The Settings pane's Contrast selector and the "Start with…", "Install updates automatically" and "Sign in through the browser" toggles, with the `settings.set` IPC command behind them. Contrast now follows the OS `prefers-contrast` setting only; start-at-login stays on the tray menu; `update.automatic` and `session.enabled` are config-file settings.

## [0.35.0] - 2026-09-02

### Added

- `SyncProgressSink` on `BridgeContext` reports sync stages to the GUI status bar and activity log. CLI callers can omit the sink; reporting failures do not fail synchronization.
- Sync-only agents answer the GUI's per-host actions through `gui::sync_only`, so an action against an agent with nothing installed locally gets a reply instead of the caller's not-found path.
- `HostEntryPayload` carries `can_verify`, `can_repair`, `can_open_config` and `can_remove`. The bindings were stale, and with no bundler or type checker in `web/js` each missing flag read as `undefined` and silently hid its button.

### Fixed

- `sync_only_verdict` moved to `integration/sync_only.rs`, the file that already owns the sync-only table, putting `agent_health.rs` back under the 300-line limit.

## [0.34.0] - 2026-09-01

### Removed

- The Home pane, the top bar's governance strip and the agent-presence dots. Home answered nothing the other panes did not, and it was where the false alarm lived — a card re-deriving a verdict the bridge had already computed. The app opens on Account; shortcuts are Ctrl+1 through Ctrl+6.
- Cowork as an agent. It is a mode of the Claude desktop app, and both of its sync emitters write into Claude Desktop's own directories, so they now key on the `claude-desktop` host. **Breaking:** `cowork` is no longer a known host id — a profile that still enables it is rejected at boot rather than ignored; drop it from `enabled_hosts`.
- The `time` crate; timestamps come from `chrono`, which the bridge already used. `serde_yaml` (archived upstream) is replaced by `serde_yaml_ng`, API-compatible.

### Added

- Terminal `login` and `install --apply` repair profiles detected as stale using the shared profile builder. Login does not enroll new hosts and skips interactive repair without a terminal; `--no-reapply` disables it explicitly.
- A re-apply's outcome is decided by re-probing the host, not by `install_profile` returning. macOS Claude Desktop installs with `open -g <mobileconfig>`, which hands the file to System Settings and returns `Ok(())` whether or not the user ever approves it under Profiles. That reports as `pending`, naming the action still outstanding, rather than as a refresh that did not happen.
- The setup screen's heading, lede and footer come from the `Brand` rather than being hard-coded: `app_name`, `docs_url` and `contact_email` ride the state wire, so a white-label build shows its own name, links its own documentation for the running platform, and points licensing at its own address with no forked component.
- GUI states include backend-computed `{ tone, code }` verdicts. The frontend renders the tone and localized message. `lint-bridge-verdicts` rejects state-name branching; `lint-bridge-i18n` checks verdict message coverage.
- MCP servers on one screen. The marketplace's MCP detail shows the live auth verdict, who the server is authenticated as, the `tools/list` result with descriptions, both URLs, and a per-server re-check (`mcp.auth.probe { serverId }`); the Status pane's MCP card is a summary that links there. The listing no longer snapshots the tool list at build time, which is why it used to show only a path.
- `just lint-bridge-globals` refuses a new `static X: OnceLock<..>` (or LazyLock, RwLock, Mutex, ArcSwap, Atomic*, Once) under `src/` outside a reasoned allowlist — tracing's dispatcher, the brand, the i18n catalogue, the two `inventory` registries, a warn-once, the keyring mirror, temp-name counters — and is red on a stale allowlist entry. `just lint-bridge-file-size` caps the web tree at 150 lines of JavaScript and 300 of CSS per file (`web/dev/` excluded); both run in `just check` and the Quality workflow.
- `just lint-bridge-layers` declares the bridge's module order and fails on any upward `crate::` reference. The bridge is one crate, so the repo's cargo-graph layer gate could not see it, and it had cycles: `integration ⇄ sync`, `integration ⇄ install`, `host_sync` naming every host, and a Codex installer raising a dialog through `gui`.
- `just bridge-bindings-check` regenerates the ts-rs bindings into scratch and diffs them against `bindings/`; a variant renamed in Rust used to leave them stale with nothing to say so.
- The whole GUI wire has a type. `StatePayload` and every payload it carries live in an unconditional `wire` module (`wire::{payloads, hosts, codes, first_run, ipc}`), so ts-rs exports them on Linux CI too — 47 bindings under `bindings/web/js/types/` instead of the five IPC envelope types, and `bridge-bindings-check` now covers the payload the front end is actually written against. `bridge.js` types `stateSnapshot()` as `Promise<StatePayload>` for editors. `gui::ipc` moved to `wire::ipc`; the Linux-only `ipc_types` alias is gone; `HostModelView` and the surface helpers are no longer GUI-gated.
- The `comms-drain` hooks (`UserPromptSubmit`, `Stop`) are installed only when the governance-owning plugin sets `hooks.comms: true` in the manifest. They rode along with every governance owner before.

- OpenCode is a supported host. The bridge writes a `provider.systemprompt` block (the OpenAI-compatible wire, the loopback `baseURL`, the negotiated model list and the `x-inference-protocol` header) and the default `model` into OpenCode's admin-managed configuration — `/etc/opencode/opencode.json`, `/Library/Application Support/opencode/opencode.json` or `%ProgramData%\opencode\opencode.json` — which OpenCode layers above every user and project file, so no local config can route inference around the gateway. The write is direct where the process may, escalates through the existing `sudo`/`osascript` path on macOS and the UAC child on Windows only when refused, and is skipped entirely when the file already says what it would say. The API key goes to the user's `auth.json` (0600); MCP connectors go to the user's global `opencode.json` and skills to `~/.config/opencode/skills`, both user-owned because unattended sync can never prompt. Skill folders are kebab-cased and the front matter `name` is forced to match, since OpenCode rejects a skill whose name differs from its folder; two ids that collapse to one folder are refused before anything is written. The probe reads the managed file and the `ai.opencode.managed` MDM domain, never user scope, and finds the `opencode` binary in the usual install prefixes even when the GUI's PATH lacks them.
- `HostApp::can_open` lets a terminal-only host say so, and the verdict then offers no Open button — Codex on Linux and every CLI host used to get one whose only outcome was an error toast.
- The Hermes card has a logo; it rendered an empty glyph. Hermes also gained the unit coverage it shipped without: probe, install/merge/remove, `.env` handling and the sync emitter.

### Changed

- **Breaking:** `HostApp::probe(&self, env: &ProbeEnv)` and `HostSync::clear(&self, ctx: &HostSyncCtx<'_>)`. A probe used to reach for the proxy port and the loopback secret through process globals; both now arrive as values, and `HostSyncCtx` carries a `loopback: &LoopbackEndpoint` so an emitter writes the same origin and bearer the caller resolved. White-label crates implementing either trait must take the new argument.
- The proxy is owned by a `BridgeContext` built once at the composition root (`cli::run_with_args`, handed to `gui::run`) and injected — `ctx.proxy.loopback()` for port, origin, MCP URLs and the bearer; `ctx.block_on` for the runtime — in place of `proxy::{handle, resolved_port, loopback_origin, loopback_bearer, mcp_url, block_on, runtime_handle, runtime_config, reload_runtime_config, start_default}` and the six `OnceLock`s behind them. `install --apply`, `sync`, `doctor` and the credential helpers build the context in attach mode (find the serving bridge's port, never bind); `proxy` and the GUI serve. Two test crates existed only because a global can show one start outcome per process; they are one crate, and one process now demonstrates serving, a sibling standing down, attaching, and the taken-default-port fallback.
- **Breaking:** `AuthProvider::authenticate(&self, session_id: &SessionId, http: &reqwest::Client)`. The gateway HTTP client is built once by the context and passed to every `GatewayClient::new(base_url, http)`; the process-global pool behind `gateway::SHARED_CLIENT` is gone, and a provider gets the client it should mint through rather than reaching for one. `auth::{acquire_bearer, obtain_live_token, read_or_refresh, mint_fresh, evaluate_chain}`, `validate::run`, `update::run_automatic` and `sync::run_once` take it too.
- The install id, the managed-MCP registry and the activity log are owned by the context as well (`ctx.install_id()`, `ctx.mcp_registry`, `ctx.activity`) and injected into the proxy server, the sync emitters (`HostSyncCtx.mcp_registry`) and the MDM renderers. `proxy::identity::install_id()`, `mcp_registry::{snapshot, rehydrate_from_disk}` without a slot, and `activity::activity_log()` no longer exist.
- The remaining service state follows: the scheduler-status cache (`ctx.schedule`, with `ScheduleStatus` now in `schedule::status`), the plugin hook-token cache (`ctx.plugin_tokens`, passed to `mint_or_refresh_plugin_token`), the Windows Start-menu probe memo (`ctx.start_menu`, carried in `ProbeEnv`), the once-per-process elevation flag, and the `--egress-allowed-hosts` override, which is now a field on `InstallOptions` instead of a set-once global (`install::set_egress_allowed_hosts` is gone; `cowork_egress_allowed_hosts` takes the flag value). `windows_policy_values` gained the allowlist as its fourth argument.
- The managed-MCP registry is loaded from disk when the context starts, in every mode. `install --apply` ran in a process that had never rehydrated it, so the Claude Code `managed-mcp.json` and the Windows `managedMcpServers` policy it wrote from the CLI were empty.
- The wire spelling of every state enum is kebab-case; `McpAuthState` and `ProxyProbeState` were bare PascalCase. The raw host probe snapshot no longer crosses the wire at all — the drawer branched on `profile_state.kind`, the same anti-pattern — and is replaced by a `health` payload of verdicts and plain facts.
- One snapshot store in the GUI. Twenty-two components each fetched the state snapshot on connect and re-fetched on whichever events they happened to know about; one module now fetches once, refreshes on every channel the bridge re-emits state for, and hands the same object to every subscriber. Every user-initiated action goes through `runAction`, so failure is never swallowed.
- `AppState` holds one lock instead of three. `mark_probing` released the snapshot lock before taking the pre-probe one, so two probes could interleave and strand the UI on `Probing`.
- The `UiEvent` name map is one exhaustive match: a new variant is a compile error, not `"Unknown"` in the logs forever. The three CLI hosts share one process finder and one config-key collector instead of three byte-identical copies each. Stringly `Result<_, String>` errors in the proxy probe, comms stream, elevated job, update args, keystore probe and Windows MDM are `thiserror` enums; the settings and session handlers stop stringifying typed errors into `io::Error`.
- The six test files under `bin/bridge/tests/` never ran in CI — the bridge shard maps to `crates/tests/unit/bridge/` only. They live there now, as the `verdicts` crate. Coverage stops ignoring `bin/bridge`. The dev fixtures every carried an `update.state` of `idle`, a phase the updater does not have; they are migrated, seeded with MCP servers in three states, and the fixture test fails on a fixture it cannot read instead of skipping it.
- Split GUI components into focused render modules and shared helpers for reconciliation, escaping, profile formatting, host actions, marketplace kinds, setup, activity and cloud status. JavaScript files and functions follow the configured size limits.
- The two oversize stylesheets are split at their component seams, and a white-label overlay (`SYSTEMPROMPT_BRIDGE_WEB_OVERLAY`) that overrode them by filename must follow: `profile.css` → `profile-layout.css`, `profile-identity.css`, `profile-usage.css`, `profile-conversations.css`; `status.css` → `status-board.css`, `kpi-card.css`, `chip.css`, `hosts-list.css`, `agents-status.css`, `mcp-auth-status.css` (the drawer's `.sp-claude__warn` rule moved to `agent-drawer.css`). `main.css` imports the new files in the positions the old ones held, so the cascade is unchanged.
- Managed skills for hosts that read `SKILL.md` folders directly (Hermes, OpenCode) go through one writer, `integration::managed_skills`, with the sidecar, pruning and front-matter rendering in one place; the Codex marketplace writer shares its renderer. Hermes's `config.yaml` MCP write is atomic, as its profile write already was.

### Fixed

- The stale-profile remediation named `install --apply`, which installed the MDM payload and the scheduled task and never touched a host profile. The advice is now true, rather than the command being wrong.
- The macOS build. `mod macos;` in the Claude Desktop host carried both a windows and a macos cfg, which are ANDed and so never true; `gui/window` used `Path` without importing it; `install/mdm/macos.rs` bound an unused `loopback` and exposed `build_prefs_plist` more publicly than its `MdmPayloadInputs` parameter. `lib.rs` gates the GUI on windows/macos, so none of it is reachable from a Linux check.

- The ts-rs bindings under `bindings/web/js/types/` were never in git — the repository's blanket `*.ts` ignore swallowed them — so `bridge-bindings-check` had nothing committed to compare against. They are un-ignored and committed.
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
- Cowork egress policy is omitted by default. Restrict it explicitly with `install --apply --egress-allowed-hosts loopback`, a comma-separated host list or `<PREFIX>_EGRESS_ALLOWED_HOSTS`. MDM snippets leave the optional setting commented.

### Fixed

- `t()` returns `undefined` for missing translations so caller fallbacks apply. Added missing Status, Marketplace and tray message keys.
- Marketplace uses a keyboard-accessible listbox with roving tabindex and category buttons in a tablist. Non-button `data-action` elements respond to Enter and Space.
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

- `HostsPayload.hosts_gated` and `AppStateSnapshot::manifest_synced` withhold host actions until a signed manifest is available. An authoritative empty enabled-host list remains distinct from unsynchronized state.
- The last-sync record is read even when the org-plugins directory does not resolve. It was nested inside that branch in `reload_into`, though nothing it restores — `last_sync_summary`, `enabled_hosts`, `host_model_protocols` — depends on the directory, so a machine without one silently lost the manifest's host gate and model protocols.
- `setup.complete` persists `onboarded.json`. State reload reads the sentinel and `auth::setup::clean` removes it with first-run metadata.

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

- Windows: a sync targeting the Cowork host from a process that cannot write the system org-plugins path now fails with elevation guidance instead of writing to a per-user directory Cowork never scans; `doctor` reports the same condition.

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
- macOS Codex probes read `config_toml_base64` from managed preferences, checking user scope before device scope. Mobileconfig installation reports the required approval in System Settings.

## [0.23.0] - 2026-08-06

### Changed

- macOS policy writes request elevation only when the file differs: terminal callers use `sudo`, GUI callers use the native administrator dialog. Cancellation is reported distinctly and retains per-plugin MCP configuration as fallback.

### Fixed

- **macOS:** updated winit to `0.31.0-beta.2` with one objc2 dependency line. GUI integration uses the revised application handler, wake-up event, boxed window and platform-attribute APIs.
- macOS: the earlier `open -g` mitigation is preserved for cleanliness (System Settings still shouldn't steal focus on profile install), but is no longer load-bearing — the crash was fixed at source by the `winit` bump.
- macOS: settings window no longer opens with `with_maximized(true)` (the initial-maximized attribute was tangled up in the earlier misdiagnosis of the resign-key crash; dropping it keeps the same 1100x760/min 800x600 default). Kept because the user can still zoom manually.
- macOS builds the WebView as a child of the winit window, preserving its native content view. Surface-resize events update WebView bounds.
- macOS: double-clicking the `.app` bundle now opens the GUI instead of emitting a JWT and exiting. `should_default_to_gui()` was `const false` off Windows; it now shares the Windows `!isatty(stdout)` heuristic so Finder/launchd invocations (no controlling terminal) route to `cmd_gui`, while Terminal invocations still fall through to `cmd_run`.

## [0.22.0] - 2026-07-30

### Breaking

- **Breaking:** gateway OAuth provisioning and plugin-token helpers accept `&BearerToken`; PAT exchange accepts `&PatToken`. Pass typed tokens rather than exposed strings to preserve redaction and zeroization.

### Fixed

- Bridge subcommands write WARN and higher diagnostics to stderr and the rolling log. INFO and lower remain file-only.
- `gateway_aligned_endpoint` aligns a loopback `token_endpoint` against the gateway actually being dialled rather than against ambient `config::load()`, which made a pure URL transform depend on process-wide state and could re-point the endpoint at a gateway the client was not talking to.
- `doctor`'s "hook token mint" warning no longer claims provisioning "runs on first sync after login". `ensure_creds` is called only from `mint_or_refresh_plugin_token`, lazily on the first plugin hook request; the old wording sent operators looking at sync for a fault that was not there.
- The `login` command's doc comment described "browser-based device-link authentication" while the implementation only stores a pasted PAT. It now says so, and records that a device certificate is the only credential that renews unattended: the proxy re-authenticates per request, so a device-link-only configuration reopens a browser on every hook and dies with `authentication timed out after 10s`.

### Added

- `login --code` exchanges an administrator-issued one-time code for a durable PAT through `/v1/auth/bridge/session-pat`. `--device-name` labels the credential and defaults to the hostname.
- Linux `install --apply` writes the environment a login shell needs instead of erroring. It emits `$XDG_CONFIG_HOME/<brand>/env.sh` exporting `ANTHROPIC_BASE_URL` (the loopback proxy origin) and `ANTHROPIC_AUTH_TOKEN`, plus a marker-delimited managed block in `~/.profile` that sources it. The token is read from the loopback key file when the file is sourced rather than baked in, so a rotated secret needs no rewrite and an absent one leaves the variable unset instead of setting an invalid credential. Re-running install replaces the block rather than appending a second one, and both files are written via temp-file + rename so a crash cannot truncate a user's dotfile. `uninstall` removes the env file and the block, leaving the rest of `~/.profile` byte-identical. Only the pair proven end-to-end is written; the `CLAUDE_INFERENCE_GATEWAY_*` keys the old snippet advertised are not.
- Linux `install --apply-schedule` registers a separate systemd user proxy service with `Restart=always`. Uninstall disables and removes it alongside the sync schedule.
- `doctor` gains three checks: whether the loopback proxy is listening (reusing `integration::proxy_probe`, which existed but was never wired in), whether the proxy's systemd unit is present and active (Linux only), and whether the `org-provisioned` marketplace is registered with the Claude Code CLI. The last names the silent failure where `sync` skips its marketplace emitter because the CLI is absent, leaving `claude plugin list` empty with every other check green.

### Changed

- **Behavior change:** Linux schedule installation retains unit files and reports manual activation commands when `systemctl` or the user bus is unavailable. This applies to both the sync timer and proxy service.
- Linux `mtls.cert_keystore_ref` resolves a device-certificate path with tilde expansion. `<PREFIX>_DEVICE_CERT` takes precedence. macOS and Windows retain their platform-specific certificate lookup; `platform_source` accepts `Option<&str>` on each platform.

## [0.21.0] - 2026-07-29

### Changed

- Dependency refresh only; no bridge source changed since 0.20.0. The lockfile moves `rmcp` from `3.0.0-beta.1` to the released `3.0.0`, which pulls `base64` 0.23.0 alongside the existing 0.22.1. `rmcp` is a transitive dependency here — the bridge declares none of it directly — so there is no behavioural change to the bridge's own surface. The release exists so the published binary matches the tree rather than a prerelease dependency.

## [0.20.0] - 2026-07-28

### Fixed

- Unset `BridgeError.detail` and IPC reply value/error fields are omitted independently of the `ts-export` feature. Consumers should accept absent or null optional fields; generated TypeScript remains optional.

### Changed

- macOS and Windows honor absolute `HOME` and XDG config/data/state overrides, otherwise using native directories. Empty and relative overrides are ignored. Windows Git Bash/MSYS environments with `HOME` set use that location.

## [0.19.0] - 2026-07-27

### Fixed

- An embedded GUI asset whose extension the server does not recognise is served as `application/octet-stream`. The fallback was `text/html; charset=utf-8`, so an unrecognised asset was handed to the webview as markup. Only names in the generated `WEB_TEXT_ASSETS` manifest reach the fallback today, so this is hardening rather than a reachable defect.
- GUI asset responses carry `x-content-type-options: nosniff`, which they omitted entirely.

### Changed

- Asset content types resolve through `systemprompt_models::mime` rather than a local table, so the GUI names types the same way the server does. JavaScript is served as `text/javascript` rather than `application/javascript`.

### Added

- Device linking provisions installed registered hosts with per-host progress and failure reporting. `first-run.json` prevents repeated provisioning on later sign-ins; auth cleanup removes it. Setup times out after five minutes without progress.

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

- OAuth client-secret storage uses `keyring-core` 1 with explicit Keychain, Windows Credential Manager and blocking Linux Secret Service stores.
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
- The host-sync registry now dedups emitters by concrete type instead of `host_id`, so the two Cowork emitters that deliberately share the `cowork` host id (plugin enables + the artifacts library) both run again.
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

- Managed plugin `version.json` uses a bundle-content hash and is written last as the completion marker. Unchanged bundles retain their files across sync polls.

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

- Windows managed-policy installation requests UAC elevation for machine-wide writes. Declined prompts and access-denied writes report the required hive and subkey.

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

- The proxy upstream client uses `Ipv4FirstResolver`, matching gateway requests and avoiding IPv6-first connection delays through WSL2 localhost forwarding.

## [0.10.2] - 2026-06-02

### Added

- **MCP authentication status in the GUI.** The Status tab gains an "MCP servers" group that runs a live `initialize` → `tools/list` round-trip per registered server through the loopback proxy (`proxy::mcp_probe`) and classifies the result — Authenticated, `bad loopback secret` (403), gateway unauthorized (401), proxy unreachable, etc. — so failures that previously required reading Cowork's `main.log` are visible in-app. Authenticated servers list the tools they expose as chips. The panel re-probes automatically after each sync and via a manual "Recheck" button. The MCP server's tools are also listed in the Marketplace detail view.

### Changed

- Managed plugins use `installationPreference: "required"`, enabling installation and restoration on the next Cowork sign-in.

### Fixed

- `gateway::Ipv4FirstResolver` no longer uses a trivial `as` cast to box its address iterator (a `Box<…> as Box<dyn …>` unsizing the newer toolchain's `trivial_casts` lint flags); the coercion is now expressed via a typed binding.

## [0.10.1] - 2026-06-02

### Fixed

- Windows managed MCP entries containing the loopback credential are written to user policy. Machine policy contains stable non-secret settings; cleanup attempts to remove legacy machine-wide MCP entries.
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

- Managed MCP entries use the bridge loopback URL and loopback-secret authorization. The proxy replaces that credential with a refreshed gateway JWT. Without a loopback secret, emitters produce an empty managed server list.

## [0.9.4] - 2026-05-28

### Breaking

- `bridge::manifest::AgentEntry.mcp_servers` and `AgentEntry.skills` are now `PluginComponentRef { source, include, exclude }` instead of `Vec<String>`. The manifest envelope tracks the unified `PluginComponentRef` shape now applied across every entity-id reference list in `systemprompt-models`. Bridge / Cowork consumers that read these fields must traverse `.include` instead of treating the value as a flat list; serialised manifests authored against 0.9.3 are no longer accepted.

### Changed

- Claude Desktop managed preferences omit `deploymentOrganizationUuid`, retaining personal connector management with the custom gateway. MCP traffic continues through the bridge proxy.
- `pick_target` no longer takes a `policy_uuid` argument and `resolve_target` no longer reads the now-absent `deploymentOrganizationUuid` policy key; Cowork plugin sync resolves the personal-session org dir directly, falling back to newest-mtime when the personal dir is missing.
- Bridge staging and metadata use platform-specific user-writable directories. Callers use `bridge_working_dir`, `bridge_staging_dir` and `bridge_metadata_dir` instead of paths under the org-plugin tree.

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

- `HostSync` defines `apply` and `clear` for manifest emitters. The central registry dispatcher selects the operation from `enabled_hosts` and logs each outcome.
- Codex managed resources use one plugin under `~/.codex/plugins/cache/systemprompt/systemprompt-managed/current/` with a manifest, skills and MCP configuration. The user config plugin toggle controls enablement; apply and clear preserve unrelated settings.
- Codex provider installation targets `/etc/codex/config.toml` on Linux/macOS in this release. It replaces bridge-owned provider, telemetry and analytics keys through a deep merge and atomic write. `CODEX_SYSTEM_CONFIG` overrides the path for tests.
- **GUI: per-host enable toggle posts to gateway (`gui/handlers/agents.rs::on_set_enabled_host_requested`).** New IPC entrypoint sends `POST /v1/bridge/enabled-hosts` with the host id and desired state, then emits `UiEvent::SetEnabledHostFinished`. The GUI no longer mutates local `agents.json` directly — host enable state is a profile fact owned by the gateway and arrives back through the next signed manifest. Matches the broader rule that host enable state lives in the user profile, not local toggles.

### Changed

- Codex profile installation separates lifecycle, merge and rendering modules and uses the standard base64 encoder. Public write behavior is unchanged.
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

- Loopback `GET`/`HEAD /healthz` and `POST /otel` paths bypass the loopback bearer check after host validation. Health is served locally; OTLP forwarding injects the gateway bearer. Shared response helpers and single-prefix bearer parsing handle the remaining proxy routes.
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
- The setup Install action enables the host, generates its profile and installs it through sequential IPC operations.
- Local proxy probes use the running proxy’s bound port, falling back to a host profile URL only when the proxy has not started.

### Added

- **Per-agent enable/disable, persisted across runs.** Every registered host (Claude Desktop, Codex CLI, …) now has an explicit `enabled` flag stored in `~/.config/systemprompt/agents.json`. Hosts default to **disabled** so a fresh install never silently probes integrations the user hasn't opted into. New IPC `agents.setEnabled({ hostId, enabled })` toggles the flag, persists it, and (when re-enabling) fires a one-shot manual probe. The host card grows an Enable/Disable button; disabled cards render as a dimmed lede with the toggle and no action buttons. `host.probe`, `host.profile.generate`, `host.profile.install`, `agent.uninstall`, and `agent.openConfig` reject disabled hosts with `Conflict`. Status summaries and the rail's agent count consider only enabled hosts.

### Fixed

- Periodic probes skip disabled hosts and log only changed profile or process state. Manual probes retain start and completion messages through `ProbeCause`.
- Sync failure and cancellation messages have localized entries. Failure output includes the error chain in both the activity view and rolling logs.
- Plugin sync verifies staged files against signed per-file SHA-256 values. Removed the additional directory-hash comparison and its helpers.
- **External link clicks could fail silently.** `gui/window/mod.rs::open_target` discarded the `Command::spawn` result, so when `xdg-open` / `cmd /C start` was missing or failed there was no record. Now logs the attempt at info level and the spawn error at error level.
- **Footer links now open via an explicit IPC instead of `target="_blank"`.** Added an `openExternalUrl` IPC command in `gui/command.rs` (HTTPS-only allowlist via `is_safe_external_url`, dispatches through the `opener` crate) and exposed it on the JS side as `bridge.openExternalUrl(url)`. `sp-footer` now handles the docs/licensing clicks through a `data-action="open-external"` delegate that calls the IPC, so the path no longer depends on the WebView's `with_new_window_req_handler` firing.
- **Footer rendered `v0.7.0 (unknown, unknown)` when `vergen` could not read git state.** `hasBuildMeta` only suppressed the literal string `"unknown"`. Added `isMissing()` to also catch empty values and unreplaced `__PLACEHOLDER__` sentinels, so the parens block disappears when build metadata is missing instead of leaking the fallbacks into the UI.
- **Help & Support section was poorly styled — buttons stretched to the drawer's right border with no breathing room.** Restyled `.sp-activity__help` in `web/css/drawer.css` as a self-contained card: outer margin so it no longer touches the drawer borders, panel background and rounded border for separation, larger gap between title and buttons, and constrained `.sp-btn-ghost` width with left-aligned labels and consistent vertical rhythm.
- **Windows GUI rendered a blank `about:blank` window.** wry 0.55 rewrites custom URI schemes to `http://<scheme>.<host>/...` on Windows/Android because WebView2 cannot register arbitrary schemes, so navigating to `sp://app/index.html` silently failed. Use `http://sp.app/index.html` on those targets and allow the rewritten origin in `allow_navigation`.
- **Native menu bar showed raw i18n keys** (`menu-edit`, `menu-view`, `menu-help`, …). The menu builder calls `i18n::t("menu-*")` but `web/i18n/en-US/bridge.ftl` had no matching entries, so the fallback returned the keys verbatim. Added the seven missing translations.
- **Re-verify button looked broken — actually silent.** Clicking "Re-verify" on a host card fired `host.probe`, ran the probe, applied the snapshot, and emitted `host.changed`, but appended nothing to the activity log. From the user's seat it looked like the click was lost. Added "[host] re-verifying…" before the spawn and "[host] re-verify complete — profile installed, process running" (or equivalent) when the snapshot is applied.
- **Bridge silently continued when the local proxy failed to start.** `gui::run` discarded the result of `proxy::start_default()` and proceeded to render the GUI even when the bind failed, so any profile generated afterwards pointed Claude Desktop / Codex at a dead `127.0.0.1:48217` (`ERR_CONNECTION_REFUSED`). Now: log success/failure to the activity drawer at startup, and refuse profile generation when the proxy isn't listening rather than handing out a profile that can't possibly work.
- Proxy startup loads or creates the loopback credential through `proxy_init`; profile generation reads the cached credential through `for_profile` and errors before proxy initialization.
- Removed unused `__TOKEN__` query placeholders from embedded asset URLs and module imports, preserving one module identity per resource.

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
- Proxy listener binds IPv4 loopback (`127.0.0.1`) first and falls back to IPv6 loopback (`::1`).
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
