//! The shell the shell tool runs, and what the model needs to know to write
//! commands for it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The family of shell the shell tool executes commands in.
///
/// One of PowerShell 7 or later (`pwsh.exe`),
/// Windows PowerShell 5.1 (`powershell.exe`), a POSIX shell (usually Git Bash
/// on Windows), or the Command Prompt (`cmd.exe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellKind {
    Pwsh,
    WindowsPowerShell,
    Bash,
    Cmd,
}

impl ShellKind {
    pub fn from_program(program: &str) -> Self {
        let file = program
            .trim()
            .trim_matches('"')
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let stem = file.strip_suffix(".exe").unwrap_or(file.as_str());
        match stem {
            "pwsh" | "pwsh-preview" => Self::Pwsh,
            "powershell" => Self::WindowsPowerShell,
            "cmd" => Self::Cmd,
            _ => Self::Bash,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pwsh => "PowerShell 7",
            Self::WindowsPowerShell => "Windows PowerShell 5.1",
            Self::Bash => "bash",
            Self::Cmd => "Command Prompt",
        }
    }

    pub const fn guidance(self) -> &'static str {
        match self {
            Self::Pwsh => PWSH_GUIDANCE,
            Self::WindowsPowerShell => WINDOWS_POWERSHELL_GUIDANCE,
            Self::Bash => BASH_GUIDANCE,
            Self::Cmd => CMD_GUIDANCE,
        }
    }
}

macro_rules! common_tail {
    () => {
        r#"
- Start long-running processes such as dev servers and file watchers as background jobs instead of waiting on them; you are told when a background job ends.
- Prefer non-interactive flags. A command that waits for keyboard input will hang until it times out.
- Use the file tools rather than shell commands to read, search or write files."#
    };
}

pub const PWSH_GUIDANCE: &str = concat!(
    r#"# Shell: PowerShell 7
The shell tool runs commands in PowerShell 7 (pwsh). Write PowerShell, not bash:
- Environment variables are $env:NAME; set one with $env:NAME = 'value'.
- Chain commands with ; to run them regardless, or with && and || to depend on the previous result.
- Single quotes make a literal string; double quotes expand variables. Run a program whose path is quoted with the call operator: & 'C:\Program Files\Tool\tool.exe' --flag.
- Discard output with | Out-Null or by assigning to $null; there is no /dev/null.
- Aliases such as ls, cat and rm exist but take PowerShell parameters: Remove-Item -Recurse -Force, not rm -rf.
- A native program's exit code is in $LASTEXITCODE."#,
    common_tail!()
);

pub const WINDOWS_POWERSHELL_GUIDANCE: &str = concat!(
    r#"# Shell: Windows PowerShell 5.1
The shell tool runs commands in Windows PowerShell 5.1 (powershell.exe). Write PowerShell, not bash, and allow for what this older version lacks:
- && and || do not exist. Chain with ; and use cmd1; if ($?) { cmd2 } when the second command must run only after the first succeeded.
- Redirection with > and Out-File writes UTF-16 by default. Pass -Encoding utf8, or create files with the write tool instead.
- Environment variables are $env:NAME; set one with $env:NAME = 'value'.
- Single quotes make a literal string; double quotes expand variables. Run a program whose path is quoted with the call operator: & 'C:\Program Files\Tool\tool.exe' --flag.
- Discard output with | Out-Null; there is no /dev/null.
- Aliases such as ls, cat and rm take PowerShell parameters: Remove-Item -Recurse -Force, not rm -rf.
- A native program's exit code is in $LASTEXITCODE."#,
    common_tail!()
);

pub const BASH_GUIDANCE: &str = concat!(
    r#"# Shell: bash
The shell tool runs commands in bash (on Windows, normally Git Bash). Write POSIX shell syntax:
- Use forward slashes. The Windows path C:\Users\me is /c/Users/me here. Quote any path with spaces.
- Windows programs run normally, but some expect Windows-style paths as arguments; pass those in single quotes so the backslashes survive.
- Environment variables are $NAME; export NAME=value sets one.
- sed, awk, find and friends are the GNU versions that ship with Git. Built-ins of cmd such as dir, copy or del are not available directly.
- Discard output with > /dev/null."#,
    common_tail!()
);

pub const CMD_GUIDANCE: &str = concat!(
    r#"# Shell: Command Prompt
The shell tool runs commands in cmd.exe. Write cmd syntax:
- Environment variables are %NAME%; set NAME=value sets one, with no spaces around the =.
- Chain with & to run regardless, or with && and || to depend on the previous result.
- Quote paths with double quotes; single quotes are not quote characters in cmd.
- Delete a file with del /q and a directory with rmdir /s /q. Discard output with > NUL.
- The shell does not expand wildcards for programs and has no PowerShell cmdlets. Keep each command simple."#,
    common_tail!()
);
