# 10. Release, and handing over the installed fleet

## 1. Release after the change

There is no harness release line. Governed device code releases inside core; the extension releases inside the template and each install.

### 1.1 Today

1. Core releases `0.N.0`: gate a frozen SHA, promote, merge, tag, publish 41 crates.
2. `release-sign.yml` builds the bridge for three systems, generates SBOMs, attests, signs, publishes a GitHub release. Homebrew and Scoop manifests follow by hand.
3. The downstream repositories land in parallel lanes: the template and each install. A fork that brands the bridge builds it from the template's `bridge/` directory at a pinned core ref.

### 1.2 After

1. Core releases `0.N.0` and publishes its crates, device crates included. **No binary is built.**
2. Template lane: bump the core pin; build every flavour including `harness`; sign and publish the reference artefacts (`harness` and `bridge`) for three systems; generate the Homebrew, Scoop, APT and RPM manifests.
3. Install lanes: bump the pin; build the server image and the branded desktop artefacts in one release, signed with the install's identity.

What disappears: the bridge workspace and lockfile, the lint mirror, the native matrix on every push, `coverage-bridge.yml`, the signing workflow in core, the second branded build, the manual manifest updates.

### 1.3 Versioning

- Device crates carry core's version. The wire version a device reports is the core version it was built from (`COMPAT_VERSION` today).
- A product's displayed version is its own, in `BrandDecl`. The bridge's check carries over: a displayed version on a different core minor from the linked crates is logged at start-up.
- A shell-only fix releases from the template without a core release.
- A wire change and its device counterpart land in one core commit and are judged together by the conformance suite on that push run.
- Changelog: device crates in core's `CHANGELOG.md` (and `changelog.d/` fragments once `future` merges); the extension in the template's.
- The semver report covers device crates like any published crate.

### 1.4 Distribution of a fork

An install's release produces a server image and up to three desktop artefacts. The instance advertises its own download and expected publisher in the well-known bridge document, so a user of that instance gets the right branded build pointed at the right default instance.

## 2. Handing over the installed bridge fleet

The one step users see. Installed fleets exist, some of more than a hundred devices. Managed fleets pin trust through MDM.

### 2.1 What exists on a machine today

| Item | Where | Handover action |
|---|---|---|
| Config, policy store | `<config_dir>/<brand>/` | Import, or keep in place (decision below) |
| Sealed PAT | Operating-system keyring | Read through the same keyring entry; no re-login |
| Loopback secret | `<config_dir>/<brand>/bridge-loopback.key`, 0600 | Keep byte for byte, so every scoped token already in a host profile stays valid |
| Pinned manifest key, `manifestTrust` | Brand policy location; machine hive on Windows | Unchanged; an administrator contract |
| Policy-writer task, `binding.json`, `accepted.json` | Task Scheduler; `%ProgramData%\<brand>\policy-writer\` | Re-register against the new binary's administrator-owned copy; `accepted.json` kept so replay protection holds |
| Scheduler entries | Task Scheduler, launchd, systemd | Re-register |
| Host profiles | Each agent's own config tree | Rewrite only the path of the credential helper and hook commands; ports and tokens unchanged |
| `org-plugins/`, sidecar ledgers | Brand data dir | Keep; the ledgers are what uninstall trusts |
| Last-sync sentinel with `AutoUpdatePolicy` | Brand data dir | Keep; update policy is fail-closed without it |
| MCP registry snapshot | Brand data dir | Keep |

**Decided: one layout.** The device build uses a single, channel-aware data root per brand (`<local data>/<vendor>/<product>[ channel]/{data,config,cache,state,tmp}` on Windows, the platform's equivalent elsewhere). An installed bridge's data is imported into it once, at the handover, and the old tree is removed. Administrator-owned locations (the machine policy location, the trust record, the policy writer's directory) do not move. The "handover action" column above reads accordingly: *import* for user data, *unchanged* for administrator locations, and the loopback secret is carried byte for byte so existing scoped tokens stay valid.

### 2.2 Self-managed machines

1. A final bridge release `0.N.x` whose updater's next version is the template-built `bridge` composition under the same product and binary name.
2. The updater stages it under the existing organisation `AutoUpdatePolicy`; a policy that withholds updates withholds this one.
3. The successor waits for the predecessor to exit (the bridge's `takeover` logic), then on first start: verifies it can read the sealed PAT and loopback secret; re-registers scheduler entries; rewrites binary paths in host profiles; runs `validate`; only then removes the old binary.
4. Any failed step leaves the old binary in place and reports through the heartbeat. A handover never leaves a machine with no working credential helper.

### 2.3 Managed fleets

1. The administrator swaps the package in MDM. Policy locations and the trust record are unchanged, so no policy edit is needed.
2. An elevated `install --apply` from the new binary re-registers the policy-writer task and refreshes the administrator-owned copy. Without elevation the writer is absent and connector changes fall back to the approval prompt, as today; `doctor` reports it.
3. Rollout is staged by the administrator; the admin Devices page shows each device's reported wire version and composition digest.

### 2.4 No fixed deprecation window

- The last bridge keeps working against instances until each known fleet's owner confirms it has moved.
- The bridge build stays in core until then.
- The gateway's minimum-client-version floor is raised per instance by its operator once its fleet has confirmed.

### 2.5 Tests before any real handover

- An upgrade test in CI: install the last bridge release, run it against the mock instance, upgrade, assert every item in 2.1 and that each configured host still reaches the proxy with its existing token.
- The manual walk in `08-platforms-and-packaging.md` section 6, including a standard user with an MDM-pinned key on Windows.
- A pilot on maintainers' devices, then one consenting install, before any larger fleet.

### 2.6 Decisions reserved to people

Any deploy, any release of the handover build, any fleet rollout and raising `min_bridge_version` on an instance are decisions for the maintainers or the install's owner, never for an agent.
