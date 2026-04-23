# Changelog

## 0.3.3 - 2026-04-23

Release-only bump — v0.3.2 tag was consumed by GitHub's immutable-releases feature before a successful publish (macos-13 runner queue, then HTTP 422 after release delete). No code changes vs 0.3.2. `release-sign.yml` now drops the Intel-mac matrix entry and creates releases atomically.

## 0.3.2 - 2026-04-23

`install --apply` on macOS supports both MDM and non-MDM workflows. `profiles install` was deprecated by Apple (macOS 11+) for CLI-initiated installs, so the default `--apply` now does a direct-write to `/Library/Managed Preferences/` — works standalone with just a sudo prompt, no profile approval UI. `--apply-mobileconfig` is the new opt-in for the MDM/System-Settings path.

- `--apply` (default): writes raw prefs plist to `/Library/Managed Preferences/com.anthropic.claudefordesktop.plist` (+ per-user path), restarts `cfprefsd`. Single sudo call.
- `--apply-mobileconfig`: builds `.mobileconfig` and `open`s System Settings → Profiles for user approval. Use this for fleet deploys via Jamf/Intune/Mosyle (distribute the file; don't try to `profiles install` it locally).
- `uninstall` mirrors: tries `profiles remove`, then sudo-removes both managed-prefs plists and kicks `cfprefsd`.
- Rejects `http://` for non-loopback gateways up front (Cowork rejects it too).

## 0.3.1 - 2026-04-23

Superseded by 0.3.2 — did not ship; `profiles install` is deprecated on modern macOS.

## 0.3.0 - 2026-04-22

Breaking: signed-manifest wire format extended with `user`, `skills`, `agents`. AgentEntry replaces `card: object` with `system_prompt: string?`. 0.2.x clients cannot deserialise 0.3.x manifests.

- `whoami` subcommand prints authenticated identity from gateway.
- `sync` materialises `user.json`, `skills/<id>/{metadata.json, SKILL.md}`, `agents/<name>.json` under `.systemprompt-cowork/`.
- `status` surfaces identity + skill/agent counts from on-disk fragments.
- Manifest signing primitive moved to `systemprompt-security::manifest_signing` (no behaviour change; same SHA-256 derivation from JWT secret, same pubkey).
- Per-user manifest assembly relocated from `systemprompt-core` gateway into the template admin extension (boundary fix — per-user tables live in the extension).

## 0.2.0 - 2026-04-22

- Renamed crate to `systemprompt-cowork` (binary `systemprompt-cowork`, lib `systemprompt_cowork`).
- Expanded scope: credential helper + plugin/MCP sync agent for Cowork's `org-plugins/` mount.
- Added `ed25519-dalek` for signed-manifest verification.
- Manual release via `cargo-zigbuild` + `gh release create` on tag `cowork-v*`; Linux x86_64 and Windows x86_64 (mingw) binaries attached. macOS binaries require a Mac host.

## 0.1.0 (unreleased)

- Initial scaffold: JSON wire contract, cache, blocking HTTP client, platform keystore trait (macOS/Windows/Linux stubs), SSO assertion fetch, stdout JSON emission.
