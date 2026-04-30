# Per-OS platform notes

Cowork is a single Rust binary with `#[cfg(target_os = "...")]` gates wherever the OSes diverge. Native code paths live under `bin/bridge/src/integration/claude_desktop/{macos,windows,shared}.rs` and `bin/bridge/src/install/{bootstrap,mdm}.rs`. The GUI lives under `bin/bridge/src/gui/` and is gated `#[cfg(any(windows, macos))]`.

## Linux (`x86_64-unknown-linux-gnu`)

- **CLI only.** No tray, no GUI, no MDM.
- `install/mdm.rs` returns an explicit error on Linux ("no documented MDM format"). Operators are expected to deploy via `/etc/...` config and a systemd unit instead.
- `install/bootstrap.rs` calls `/usr/sbin/chown` after creating `org-plugins/` to restore ownership from root → invoking user when the install ran via `sudo`.
- Plugin mount path follows XDG conventions; consult `bin/bridge/README.md` for the exact directory.
- Built and signed by `release-sign.yml` on `ubuntu-latest`.

## macOS (`aarch64-apple-darwin`, `x86_64-apple-darwin`)

- **GUI**: tray icon + native settings window via `WKWebView`. Code under `bin/bridge/src/gui/`.
- **Managed Preferences**: read from `/Library/Managed Preferences/$USER/<domain>.plist` (user-scoped) with system-path fallback. Implemented in `src/integration/claude_desktop/macos.rs`.
- **MDM**: generates a `.mobileconfig` profile via template; operators import it through their MDM (Jamf, Kandji, Mosyle, etc.). `apply_mdm()` in `src/install/mdm.rs` dispatches to `super::macos::apply()`.
- **Process discovery**: spawns `/bin/ps` to list Claude processes.
- **`.app` bundle**: produced by `bin/bridge/scripts/make-mac-app.sh`. `make-icons.swift` generates `AppIcon.icns` from the source PNGs. Wrapped binary lives at `Contents/MacOS/systemprompt-cowork` with `Info.plist`.
- **Tag tracks**:
  - `aarch64-apple-darwin` rides the main `v*` matrix in `release-sign.yml`.
  - `x86_64-apple-darwin` is **separate**: tagged `cowork-mac-v*` in core, built by a dedicated workflow, and bundled into the user-facing template release as a zip via the `cowork_mac_x64_tag` workflow input.
- **Notarization**: not currently performed by CI. Cosign keyless signing is the primary integrity guarantee. Distribution channels (Homebrew, Scoop) handle their own packaging-side verification.

## Windows (`x86_64-pc-windows-msvc`)

- **GUI**: tray icon + native settings window via `WebView2`. Code under `bin/bridge/src/gui/`.
- **Managed Preferences**: read from `HKLM\SOFTWARE\Policies\Claude` (machine, elevated) and `HKCU\SOFTWARE\Policies\Claude` (user). Implemented in `src/integration/claude_desktop/windows.rs`.
- **MDM**: writes registry values (`REG_SZ`, `REG_DWORD`) under HKLM (requires elevation) or HKCU. `apply_mdm()` dispatches to `super::windows::apply()`.
- **Process discovery**: `tasklist /FO CSV` and `GetConsoleProcessList` via `bin/bridge/src/winproc.rs`. `winproc` also handles console attachment so the credential-helper mode prints to the parent console when invoked from a terminal.
- **PE metadata + icon**: `bin/bridge/build.rs` runs only on Windows, uses `winresource` to embed `bin/bridge/assets/app-icon.ico` and set `FileDescription` / `ProductName` / `CompanyName` in the EXE header.
- **WebView2 runtime**: required at runtime; modern Windows installs ship it. Fallback installer prompt is on the roadmap, not currently implemented.

## Adding a new platform-specific feature

1. Pick the right file:
   - Claude Desktop integration → `src/integration/claude_desktop/{macos,windows,shared}.rs`
   - Install/bootstrap behavior → `src/install/{bootstrap,mdm}.rs`
   - Process / OS calls → new module gated `#[cfg(target_os = "...")]`, mirror `winproc.rs` style on Windows.
2. Gate every new platform module with `#[cfg(target_os = "...")]` at the `mod` declaration; do not gate inside the body.
3. If the feature is part of MDM, add a dispatch arm in `apply_mdm()` (and a friendly error on platforms that don't support it).
4. If the feature changes what's possible per OS, update the capability matrix in [architecture.md](architecture.md).
5. If the feature requires a new Windows asset (icon variant, manifest), add it under `bin/bridge/assets/` and reference it from `build.rs`.
6. CI: the existing 3-OS matrix in `release-sign.yml` will pick the new code up automatically. Add a smoke test under `bin/bridge/tests/` if behavior is observable from the binary.
