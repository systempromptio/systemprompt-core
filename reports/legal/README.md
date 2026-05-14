# Third-Party License Inventory — systemprompt-core

Generated: 2026-05-14
Version: 0.9.2
Tools: `cargo-deny 0.19.0`, `cargo-about 0.9.0`

## Executive summary

**All dependency licenses are commercially compatible with sale/relicensing of this codebase.** `cargo deny check` passes against the project's allow-list (`deny.toml`): `advisories ok, bans ok, licenses ok, sources ok`.

229 unique third-party crates resolved. No GPL, AGPL, or SSPL dependencies. No copyleft licenses beyond MPL-2.0 (weak, file-level).

## License distribution (SPDX expression occurrences)

| SPDX | Crate count | Class | Notes |
|---|---|---|---|
| MIT | 440 | Permissive | Most common. |
| Apache-2.0 | 351 | Permissive | Often dual-licensed with MIT. |
| BUSL-1.1 | 31 | Source-available | **Project's own crates** (`systemprompt-*`). Seller holds copyright. |
| Unicode-3.0 | 19 | Permissive | ICU / Unicode data. |
| Zlib | 9 | Permissive | |
| ISC | 8 | Permissive | `ring`, `rustls`, `maxminddb` (reader only — see GeoLite2 note). |
| BSD-3-Clause | 7 | Permissive | `curve25519-dalek`, `ed25519-dalek`, `subtle`. |
| Unlicense | 7 | Public-domain dedication | Dual-licensed (MIT/Apache also available). |
| MPL-2.0 | 6 | **Weak copyleft** | `webauthn-rs` family, `option-ext`. File-level; commercial use permitted; modifications to those specific files must remain MPL-2.0. |
| BSL-1.0 | 5 | Permissive | Boost. |
| BSD-2-Clause | 3 | Permissive | `comrak`. |
| CDLA-Permissive-2.0 | 2 | Permissive | `webpki-roots`. |
| LGPL-2.1-or-later | 2 | Copyleft (avoided) | `r-efi` is tri-licensed `MIT OR Apache-2.0 OR LGPL-2.1-or-later`; we elect MIT/Apache. Not LGPL-bound. |
| 0BSD | 1 | Permissive | |
| BSD-1-Clause | 1 | Permissive | `fiat-crypto` (also Apache-2.0 available). |
| Unicode-DFS-2016 | 1 | Permissive | `finl_unicode`. |

Counts reflect SPDX *expression* occurrences across dual/tri-licensed crates, not unique-crate totals.

## Items requiring buyer disclosure

1. **Project license is BUSL-1.1.** Seller's own code (`systemprompt-*`) is published under BUSL-1.1. The seller, as copyright holder, retains the right to relicense and assign. Crates already on crates.io under BUSL remain so for existing consumers; this does not affect sale of the copyright.

2. **MPL-2.0 dependencies** (`webauthn-rs`, `webauthn-rs-core`, `webauthn-attestation-ca`, `webauthn-rs-proto`, `base64urlsafedata`, `option-ext`) — weak file-level copyleft. Permits commercial distribution and linking; modifications to those specific files must be published under MPL-2.0. No impact on proprietary code that uses them as libraries.

3. **MaxMind GeoLite2 data files**, if shipped with deployments, are under MaxMind's separate EULA — not Apache-2.0. The `maxminddb` Rust crate (Apache-2.0/ISC) is the reader only. **Action:** verify no `.mmdb` data files are bundled in distributed artefacts.

4. **OpenSSL exception in `ring`** — clarified as "ISC AND MIT AND OpenSSL" by convention. Standard in the Rust crypto ecosystem.

## Files in this directory

Only `README.md` is tracked in git. All other artefacts are regenerated on demand and ignored via `.gitignore`:

- `licenses.html` — full third-party notices, suitable for distribution attribution (~830 KB).
- `cargo-deny-licenses.txt` — `cargo deny check licenses` output.
- `cargo-deny-full.txt` — `cargo deny check` summary line.
- `spdx-by-license.txt` — crates grouped by SPDX license.
- `spdx-by-crate.txt` — crates with their SPDX expressions.

## Reproducing

See [`tools/license-audit/README.md`](../../tools/license-audit/README.md) for commands and tool installation.

## Known advisory ignores

- **RUSTSEC-2023-0071** (Marvin timing attack in `rsa` via `jsonwebtoken`) — accepted with documented mitigation; no upstream fix. See `deny.toml` for rationale. Recommend ES256/ES384/EdDSA over RSA-family JWT signing.
