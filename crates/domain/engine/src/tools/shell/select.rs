//! Choosing the shell commands run in. On Windows the order is `PowerShell` 7,
//! Git Bash, Windows `PowerShell` 5.1, then cmd; elsewhere bash, then sh. A
//! configured shell wins when it resolves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::tokenize::Dialect;

/// The family of shell a command runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellKind {
    Pwsh,
    WindowsPowerShell,
    GitBash,
    Cmd,
    Posix,
}

impl ShellKind {
    pub const fn dialect(self) -> Dialect {
        match self {
            Self::Pwsh | Self::WindowsPowerShell => Dialect::PowerShell,
            Self::Cmd => Dialect::Cmd,
            Self::GitBash | Self::Posix => Dialect::Posix,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pwsh => "PowerShell 7",
            Self::WindowsPowerShell => "Windows PowerShell 5.1",
            Self::GitBash => "Git Bash",
            Self::Cmd => "cmd.exe",
            Self::Posix => "a POSIX shell",
        }
    }

    pub(crate) const fn notes(self) -> &'static str {
        match self {
            Self::Pwsh => {
                "This is PowerShell 7. `&&` and `||` chain commands; `$env:NAME` reads an environment variable; use \
                 cmdlets such as Get-ChildItem, Test-Path and Remove-Item. Run a program whose path has spaces with \
                 `& \"C:\\path\\tool.exe\" args`. Single quotes make literal strings; the escape character is the backtick."
            },
            Self::WindowsPowerShell => {
                "This is Windows PowerShell 5.1, which has no `&&` or `||`: run a dependent command as \
                 `first; if ($?) { second }`. It also lacks `??`, `?.` and the ternary operator. `>` and Set-Content \
                 do not write UTF-8 unless you add `-Encoding utf8`. `$env:NAME` reads an environment variable; the \
                 escape character is the backtick."
            },
            Self::GitBash => {
                "This is Git Bash, a POSIX shell on Windows. Use forward slashes and write drive paths as \
                 `/c/Users/...`. Windows programs can be run directly; `cmd //c` reaches cmd built-ins."
            },
            Self::Cmd => {
                "This is cmd.exe. Chain with `&&` and `||`, read variables as `%NAME%`, put paths with spaces in \
                 double quotes, and use `call` to run batch files."
            },
            Self::Posix => {
                "Chain dependent commands with `&&` and quote paths that contain spaces."
            },
        }
    }
}

/// A resolved shell executable and its kind. `detect` takes the configured
/// shell when it resolves and the platform's preferred one otherwise; `resolve`
/// takes a path, or a bare name looked up on PATH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSpec {
    pub path: PathBuf,
    pub kind: ShellKind,
}

impl ShellSpec {
    pub fn detect(preferred: Option<&Path>) -> Self {
        if let Some(path) = preferred {
            if let Some(spec) = Self::resolve(path) {
                return spec;
            }
            tracing::warn!(shell = %path.display(), "configured shell not found; using the detected one");
        }
        platform_default()
    }

    pub fn resolve(path: &Path) -> Option<Self> {
        let found = if path.is_absolute() || path.components().count() > 1 {
            path.is_file().then(|| path.to_path_buf())
        } else {
            which::which(path).ok()
        };
        found.map(|path| {
            let kind = kind_of(&path);
            Self { path, kind }
        })
    }
}

pub(crate) fn kind_of(path: &Path) -> ShellKind {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match stem.as_str() {
        "pwsh" => ShellKind::Pwsh,
        "powershell" => ShellKind::WindowsPowerShell,
        "cmd" => ShellKind::Cmd,
        _ if cfg!(windows) => ShellKind::GitBash,
        _ => ShellKind::Posix,
    }
}

#[cfg(windows)]
fn platform_default() -> ShellSpec {
    if let Ok(path) = which::which("pwsh") {
        return ShellSpec {
            path,
            kind: ShellKind::Pwsh,
        };
    }
    if let Some(path) = git_bash() {
        return ShellSpec {
            path,
            kind: ShellKind::GitBash,
        };
    }
    if let Ok(path) = which::which("powershell") {
        return ShellSpec {
            path,
            kind: ShellKind::WindowsPowerShell,
        };
    }
    let path = which::which("cmd").unwrap_or_else(|_| PathBuf::from("cmd.exe"));
    ShellSpec {
        path,
        kind: ShellKind::Cmd,
    }
}

// Why: System32\bash.exe is the WSL launcher, so Git Bash is found next to
// git.exe rather than on PATH.
#[cfg(windows)]
fn git_bash() -> Option<PathBuf> {
    if let Ok(git) = which::which("git") {
        for root in git.ancestors().skip(1).take(3) {
            let candidate = root.join("bin").join("bash.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|candidate| candidate.is_file())
}

#[cfg(not(windows))]
fn platform_default() -> ShellSpec {
    let path = which::which("bash").unwrap_or_else(|_| PathBuf::from("/bin/sh"));
    ShellSpec {
        path,
        kind: ShellKind::Posix,
    }
}
