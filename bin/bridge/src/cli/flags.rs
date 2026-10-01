//! The flags each CLI command accepts, checked before the command runs.
//!
//! A command missing from this table is not a command: dispatch reports it
//! as unknown. Keep each entry in step with the command's own parsing and the
//! help text in `lib.rs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::args::CommandFlags;

const NONE: CommandFlags = CommandFlags {
    switches: &[],
    values: &[],
    positionals: 0,
    inline_values: false,
};

#[must_use]
pub fn command_flags(command: Option<&str>) -> Option<CommandFlags> {
    Some(match command? {
        "run" | "proxy" | "feedback-status" | "logout" | "clean" | "status" | "whoami"
        | "validate" | "comms-drain" | "diagnostics" | "doctor" | "gui" => NONE,
        "device-enroll" => CommandFlags {
            values: &["--token-file"],
            ..NONE
        },
        "login" => CommandFlags {
            switches: &["--stdin", "--no-browser", "--no-reapply"],
            values: &["--gateway", "--device-name", "--code"],
            positionals: 1,
            ..NONE
        },
        "install" => CommandFlags {
            switches: &["--apply", "--apply-mobileconfig", "--apply-schedule"],
            values: &[
                "--gateway",
                "--pubkey",
                "--egress-allowed-hosts",
                "--host",
                "--hosts",
                "--print-mdm",
                "--emit-schedule-template",
            ],
            ..NONE
        },
        "__install-claude-policy" => CommandFlags {
            positionals: 2,
            ..NONE
        },
        "__apply-policy-task" => CommandFlags {
            positionals: 1,
            ..NONE
        },
        "sync" => CommandFlags {
            switches: &[
                "--watch",
                "--fresh",
                "--allow-unsigned",
                "--force-replay",
                "--allow-tofu",
            ],
            values: &["--interval"],
            ..NONE
        },
        "update" => CommandFlags {
            switches: &["--check", "--yes", "-y"],
            ..NONE
        },
        "oauth-client" => CommandFlags {
            positionals: 1,
            ..NONE
        },
        "uninstall" => CommandFlags {
            switches: &["--purge"],
            values: &["--host"],
            ..NONE
        },
        "credential-helper" => CommandFlags {
            values: &["--host"],
            inline_values: true,
            ..NONE
        },
        "dev-web" => CommandFlags {
            values: &["--port", "--web-root"],
            ..NONE
        },
        _ => return None,
    })
}
