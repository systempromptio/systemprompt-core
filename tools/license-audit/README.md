# License audit tooling

Configuration for the third-party license inventory. Ownership means proving what runs in your binary. This tooling records every dependency license for sale, redistribution, and compliance review.

## Files

- `about.toml` — accepted licenses + clarifications for `cargo-about`
- `about.hbs` — Handlebars template producing the HTML attribution report

The advisory and license gate configuration lives at the repo root in `deny.toml` (standard `cargo-deny` location, referenced by CI).

## Regenerate the inventory

```bash
cargo deny check
cargo deny list -l license > reports/legal/spdx-by-license.txt
cargo deny list -l crate   > reports/legal/spdx-by-crate.txt
cargo about generate \
  tools/license-audit/about.hbs \
  --config tools/license-audit/about.toml \
  -o reports/legal/licenses.html
```

Generated artefacts are written to `reports/legal/` and are not committed. Regenerate before any release or data-room handover.

## Installing the tools

```bash
cargo install cargo-deny  --locked
cargo install cargo-about --locked --features cli
```

Part of [systemprompt.io](https://systemprompt.io), self-hosted AI governance infrastructure.
