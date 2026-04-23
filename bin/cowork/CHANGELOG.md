# Changelog

## 0.3.1 - 2026-04-23

Fix: `install --apply` on macOS now installs a `.mobileconfig` into `/Library/Managed Preferences/` via `sudo profiles install`, which is the domain Cowork actually reads. Previously wrote to per-user defaults, which Cowork ignored.

- `apply_macos`: build mobileconfig plist with `PayloadScope=System`, write to tempfile, `sudo profiles install -path`. Rejects `http://` for non-loopback hosts up front.
- `uninstall` on macOS: runs `sudo profiles remove -identifier io.systemprompt.cowork.mdm` to restore cloud mode.
- Stable PayloadIdentifier `io.systemprompt.cowork.mdm` + deterministic UUIDs keep re-applies idempotent.

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
