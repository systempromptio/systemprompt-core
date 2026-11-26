# 07. The extension in the template

Decision D7: standard views live in the template, forked, MIT, with the same architecture existing installations use.

## 1. Layout

```
extensions/
  web/      shared, admin, content, site, jobs        existing
  mcp/      systemprompt, shared                      existing
  cli/      kit-export                                existing
  harness/
    shared/     tokens, components, markdown, diff, strings     from harness-ui, desktop/strings
    shell/      window, frame, mount points, tray, deep links    from desktop/app, desktop/state
    views/      transcript, composer, docks, pickers, settings,  from desktop/views
                sign-in, governance, drive,
                hosts, marketplace, first run, inbox             new, replacing the bridge window
    theme/      ThemeDecl palettes                               from harness-ui/theme/palettes
    brand/      BrandDecl, icons                                 from sp-brand, bridge brand.rs
flavours/
  harness/      flavour.yaml, README.md, setup
packaging/      windows, macos, linux; reads `package --describe`
```

- Each directory under `extensions/harness/` is a workspace member with its own `src/extension.rs` holding its contributions.
- Dependencies inside it follow the web extension's shape: `shared` at the bottom; `shell`, `theme`, `brand` on `shared`; `views` on `shell`.
- Only these crates link the window toolkit. `views` never names a toolkit type outside `shared`'s re-exports, so a toolkit bump touches one crate's surface.
- No UI text literal in a view; every string comes from the strings bundle.

## 2. The binary

- A second binary target beside the server's, with its own lockfile, so the server image never compiles the window toolkit and the device build never compiles the server closure.
- `main.rs` is one line: `composition().run()`.
- The composition names core's slices by feature and adds the extension's members.

The template ships two named compositions, both built on every push. A product name is a brand declaration, not a codebase:

| Composition | Slices | Purpose |
|---|---|---|
| `harness` | Reference harness, plus `proxy` and the `host-*` slices | The full product |
| `bridge` | `device`, `proxy`, every `host-*`, `machine`, `tray`, `update` | The bridge's successor, under the existing `systemprompt-bridge` product and binary name |

## 3. Flavour

`harness` joins `gateway`, `mcp`, `agents`, `site`, `full` in `flavours/index.yaml`. It differs from the server flavours in kind (it produces installers, not an image), so `scripts/flavour.py` gains an `artefact` field (`image` or `desktop`) and `flavour-build.sh` and `flavour-smoke.sh` branch on it. The smoke test launches the packaged app against the mock instance and reaches the sign-in gate.

## 4. What a fork owns, and what it must not touch

| A fork changes freely | A fork cannot change |
|---|---|
| Brand, icons, product name, default instance, displayed version | The governor, accounts, manifest trust, the loopback secret |
| Themes and type scale | Reserved CLI paths and their exit codes |
| Any view; the layout of the frame within the mount points | The sign-in gate and governance page exist and are reachable |
| Its own tools, agents, commands, host integrations, governor layers | A verdict can only be narrowed |
| Which optional core slices are linked | Required members named by the instance |

The fork contract suite (`09-quality-and-gates.md`) checks the right-hand column on the fork's own composition.

## 5. Installs

- Installs fork `extensions/harness/` like any other extension path and diverge on purpose. Duplicated shell code between installs is correct by design.
- An install's `harness` brand member replaces the template's. The second brand compiled into the harness repository today moves to the install that owns it.
- The fork-drift gate that compares installs with the template covers `extensions/harness/` and `packaging/`. It reports drift from the template's scaffolding, not divergence in product code.
- Validation uses the existing rule: point at local core with `[patch.crates-io]` through `scripts/core-patch.sh`; never bump pins to validate.
- An install's release builds its server image and its desktop artefacts together (`10-release-and-fleet-handover.md`).

## 6. Licence

The template is MIT. `extensions/harness/*` is MIT. It links core's BUSL-1.1 crates, exactly as `extensions/web` does today. The harness repository's `desktop`, `ui` and `brand` crates are relicensed when they move (`06-harness-repo-disposition.md` section 4).

## 7. Toolkit

`gpui-kit` is pinned at `=0.9.1`. Five unmaintained-crate advisories arrive only through it; they are recorded in the template's `deny.toml` with reasons, not core's. Type-erased contributed views and Linux (Wayland and X11) support are **unverified** for this pin. S3 prototypes one contributed view. Linux desktop is a requirement: if the pin does not support it, S5 raises or patches the pin and re-checks every view.
