# systemprompt-cowork

Credential helper and plugin/MCP sync agent for Anthropic's Claude Cowork, targeting the systemprompt.io gateway.

**Two roles in one binary:**

1. **Credential helper.** Run with no arguments. Prints a single JSON object to stdout matching Anthropic's `inferenceCredentialHelper` contract:

   ```json
   {"token": "...", "ttl": 3600, "headers": {}}
   ```

2. **Sync agent.** Pulls the signed plugin + MCP allowlist manifest for the authenticated user into Cowork's `org-plugins/` directory, OS-appropriate path.

All diagnostics go to stderr. Exit code 0 on success, non-zero on failure.

## Commands

| Command | Purpose |
|---------|---------|
| _(no args)_ | Default: emit credential-helper JSON |
| `login <sp-live-…>` | Store a PAT securely |
| `logout` | Remove the stored PAT |
| `status` | Show config, cache, last sync, installed plugin count |
| `install` | Bootstrap Cowork integration on this machine; print MDM snippets |
| `sync` | Pull plugins + MCP allowlist from gateway into `org-plugins/` |
| `validate` | End-to-end self-check |
| `uninstall` | Reverse install; `--purge` also clears tokens |

## Build

This crate is **not** part of the main workspace. Build standalone:

```bash
just build-cowork                              # host triple
just build-cowork aarch64-apple-darwin         # cross target
just build-cowork-all                          # mac arm+x86, windows x86_64, linux x86_64
```

## Runtime environment

| Variable                       | Purpose                                                |
|--------------------------------|--------------------------------------------------------|
| `SP_COWORK_GATEWAY_URL`        | Gateway base URL (default `https://gateway.systemprompt.io`) |
| `SP_COWORK_USER_ASSERTION`     | Override for SSO assertion (dev only)                  |
| `SP_COWORK_DEVICE_CERT`        | Linux dev path to device cert (dev only)               |
| `SP_COWORK_CONFIG`             | Path to `systemprompt-cowork.toml` (default: `<config_dir>/systemprompt/systemprompt-cowork.toml`) |

Cache lives at the OS cache dir under `systemprompt-cowork/cache.json` (mode 0600 on unix).

## Plugin mount paths (Cowork-managed)

| OS | Path |
|----|------|
| macOS | `/Library/Application Support/Claude/org-plugins/` (system) · `~/Library/Application Support/Claude/org-plugins/` (user fallback) |
| Windows | `C:\ProgramData\Claude\org-plugins\` (system) · `%LOCALAPPDATA%\Claude\org-plugins\` (user fallback) |
| Linux | `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/` — convention, not documented by Anthropic |

## Release

Tag `cowork-vX.Y.Z` triggers `.github/workflows/cowork-release.yml` which builds binaries for macOS (arm64 + x86_64), Windows (x86_64), and Linux (x86_64) and attaches them to a GitHub Release. Core's normal CI is untouched by this tag.
