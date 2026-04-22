# sp-cowork-auth

Credential helper binary for Cowork / Claude Code clients targeting the systemprompt.io gateway.

Run with no arguments. On success, prints a single JSON object to stdout:

```json
{"token": "...", "ttl": 3600, "headers": {}}
```

All diagnostics go to stderr. Exit code 0 on success, non-zero on failure.

## Build

This crate is **not** part of the main workspace. Build standalone:

```bash
just build-cowork-auth                              # host triple
just build-cowork-auth aarch64-apple-darwin         # cross target
just build-cowork-auth-all                          # mac arm+x86, windows x86_64
```

## Runtime environment

| Variable                       | Purpose                                                |
|--------------------------------|--------------------------------------------------------|
| `SP_COWORK_GATEWAY_URL`        | Gateway base URL (default `https://gateway.systemprompt.io`) |
| `SP_COWORK_USER_ASSERTION`     | Override for SSO assertion (dev only)                  |
| `SP_COWORK_DEVICE_CERT`        | Linux dev path to device cert (dev only)               |

Cache lives at the OS cache dir under `systemprompt-cowork-auth/cache.json` (mode 0600 on unix).

## Release

Tag `cowork-auth-vX.Y.Z` triggers `.github/workflows/cowork-auth-release.yml` which builds signed binaries for macOS (arm64 + x86_64) and Windows (x86_64) and attaches them to a GitHub Release. Core's normal CI is untouched by this tag.
