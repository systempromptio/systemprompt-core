//! The `help` text, one section per command family.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::brand::{self, Brand};

#[must_use]
pub(crate) fn help() -> String {
    let b = brand::brand();
    [
        credential_commands(b),
        sync_commands(b),
        maintenance_commands(),
        env_overrides(b),
    ]
    .concat()
}

fn credential_commands(b: &Brand) -> String {
    format!(
        "{bin} <command>

Commands (credential helper):
  run                        (default) Emit JWT envelope to stdout
  proxy                      Run the local inference proxy headlessly (Linux/server
                             equivalent of the desktop GUI). Listens on
                             127.0.0.1:48217, swaps a loopback secret for a fresh
                             gateway JWT, injects identity headers, and refreshes
                             in the background. Point ANTHROPIC_BASE_URL /
                             ANTHROPIC_AUTH_TOKEN at the printed values.
  login [<sp-live-...>]      Store a PAT securely and wire up {config_file}.
    [--stdin]                 Read the PAT from stdin instead of the command line
                              (preferred: argv is visible to other processes).
    [--code <exchange-code>]  Redeem an administrator-issued one-shot code.
                              With neither, signs in through the browser.
    [--gateway <url>]
  logout                     Remove the stored PAT and its config section
  clean                      Wipe all local {bin} state (config + PAT + token cache).
                             Returns the GUI to a fresh splash. Does not touch
                             org-plugins or managed profiles — see `uninstall --purge`.
  status                     Show config paths and what is currently set up
  whoami                     Print authenticated identity from the gateway

",
        bin = b.binary_name,
        config_file = b.config_file,
    )
}

fn sync_commands(b: &Brand) -> String {
    format!(
        "Commands (plugin + MCP sync):
  install                    Bootstrap Bridge integration on this machine
    [--gateway <url>]                     Persist gateway URL
    [--pubkey <base64>]                   Pin manifest signing pubkey out of band.
                                          With --apply, also written to
                                          HKLM\\SOFTWARE\\Policies\\Claude
                                          (Windows) or the Managed Preferences plist
                                          (macOS) so MDM can roll it to a fleet.
    [--apply]                             Apply locally (Windows registry / macOS
                                          Managed Preferences direct-write). No MDM
                                          needed — works for a single-user dev setup.
    [--apply-mobileconfig]                (macOS) Build .mobileconfig and open System
                                          Settings → Profiles for user approval.
                                          Use when the fleet is MDM-managed or Apple's
                                          approval UI is required.
    [--egress-allowed-hosts <csv>]        Restrict which hosts Cowork may reach.
                                          Omitted by default, which leaves Cowork's
                                          own unrestricted egress in force. Pass
                                          `loopback` for the air-gapped posture
                                          (127.0.0.1 only), or a comma-separated
                                          host list. Also read from
                                          {egress_env}.
    [--host <id>]                         Enrol a host application: write its
                                          provider profile against this proxy's
                                          port and loopback secret, and its API
                                          key. Repeatable, and accepts a comma-
                                          separated list. Without it `install`
                                          enrols nothing and a client such as
                                          OpenCode gets MCP servers from `sync`
                                          but no credential, which the proxy
                                          rejects with 403.
                                          Ids: claude-code, claude-desktop,
                                          codex, codex-cli, opencode,
                                          opencode-cli, hermes.
    [--hosts all]                         Enrol every host this build offers.
                                          Undo one with
                                          `uninstall --host <id>`, which touches
                                          only that host and leaves the bridge
                                          installed.
    [--print-mdm macos|windows|linux]     Print the MDM snippet for that OS
    [--emit-schedule-template macos|windows|linux]
                                          Write an OS scheduler template to CWD
    [--apply-schedule]                    Register the periodic sync job with this
                                          machine's scheduler (launchd / Task
                                          Scheduler / systemd --user). Idempotent;
                                          `uninstall` deregisters it.
                                          Config keys ({config_file}): gateway_url,
                                          deployment_organization_uuid, [sync.trust]
                                          (gateway, key, source — the operator pin),
                                          and [cowork] session_org_dir — pins the Cowork
                                          session directory to sync into instead of
                                          resolving it. See README.
  sync                       Pull plugins + MCP allowlist from gateway into org-plugins
    [--watch] [--interval <secs>] [--fresh] [--allow-unsigned] [--force-replay] [--allow-tofu]
                                          --fresh asks the gateway to rebuild the
                                          catalogue instead of answering from its
                                          per-user memo (a connector linked just now).
                                          --allow-tofu opts back into trust-on-first-use
                                          pubkey fetch when no pinned key is available;
                                          required only if MDM rollout is unavailable.
  drive                      Two-way sync of the workspace Drive folder
    status                              Folder, pending uploads and open conflicts
    sync                                Run one pull-then-upload cycle now
    conflicts                           List local edits kept as conflict siblings
  oauth-client               Manage the per-tenant OAuth client used to mint
                             plugin-scoped hook tokens
    status                              Show locally-stashed creds (no secret echo)
    rotate                              Force re-provision; new client_secret minted
",
        config_file = b.config_file,
        egress_env = b.env("EGRESS_ALLOWED_HOSTS"),
    )
}

fn maintenance_commands() -> String {
    "  update                     Install the newest build the gateway publishes, verifying
                             its checksum (and, on macOS, its signature) first
    [--check]                             Report only; exit 1 if an update is available
    [--yes]                               Skip the confirmation prompt
  validate                   End-to-end self-check (paths, gateway, creds, signatures)
  doctor                     Diagnose common bridge failure modes (config, creds, gateway,
                             loopback secret, pinned pubkey) with one line per check
  uninstall                  Reverse install (metadata + staging)
    [--purge]                             Also remove stored PAT/credentials
  device-enroll --token-file PATH          Enroll an administrator-issued device credential
  feedback-status                         Show installation receipt acknowledgment status
  dev-web [--port N]         (dev builds only) Serve the GUI web tree over HTTP
    [--web-root DIR]         so a browser can render it on a machine with no
                             webview. See bin/bridge/README.md.
  gui                        Launch the native settings UI (Windows + macOS)
  help                       Show this help

"
    .to_owned()
}

fn env_overrides(b: &Brand) -> String {
    format!(
        "Env overrides:
  {config_env}           Path to {config_file}
  {pat_env}              Inline PAT (overrides file-based [pat])
  {egress_env}
                             Comma-separated Cowork egress allowlist; `loopback`
                             expands to 127.0.0.1. Unset means no restriction.
",
        config_file = b.config_file,
        egress_env = b.env("EGRESS_ALLOWED_HOSTS"),
        config_env = b.env("CONFIG"),
        pat_env = b.env("PAT"),
    )
}
