//! Pure assembly and escaping for the privileged shell scripts the macOS
//! elevation path executes via `sudo` or `osascript`.
//!
//! Kept free of platform gates so every platform's test suite covers the
//! exact bytes handed to the elevated shell.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

// Why: an AppleScript string literal cannot contain a raw newline, and every
// caller passes a multi-line `set -e` shell script — newlines must become the
// literal's `\n` escape or osascript rejects the whole program.
#[must_use]
pub fn applescript_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            other => out.push(other),
        }
    }
    out
}

#[must_use]
pub fn shell_quote(s: &str) -> String {
    let escaped = s.replace('\'', r"'\''");
    format!("'{escaped}'")
}

#[must_use]
pub fn write_policy_script(
    dir: &Path,
    staged_mcp: &Path,
    mcp: &Path,
    staged_settings: &Path,
    settings: &Path,
) -> String {
    format!(
        "set -e\n\
         /bin/mkdir -p {dir}\n\
         /usr/bin/install -m 0644 {staged_mcp} {mcp}\n\
         /usr/bin/install -m 0644 {staged_settings} {settings}\n",
        dir = shell_quote(&dir.to_string_lossy()),
        staged_mcp = shell_quote(&staged_mcp.to_string_lossy()),
        mcp = shell_quote(&mcp.to_string_lossy()),
        staged_settings = shell_quote(&staged_settings.to_string_lossy()),
        settings = shell_quote(&settings.to_string_lossy()),
    )
}

#[must_use]
pub fn clear_policy_script(
    remove_mcp: Option<&Path>,
    staged_settings: Option<(&Path, &Path)>,
) -> String {
    let mut script = String::from("set -e\n");
    if let Some(path) = remove_mcp {
        script.push_str(&format!(
            "/bin/rm -f {}\n",
            shell_quote(&path.to_string_lossy())
        ));
    }
    if let Some((staged, dest)) = staged_settings {
        script.push_str(&format!(
            "/usr/bin/install -m 0644 {} {}\n",
            shell_quote(&staged.to_string_lossy()),
            shell_quote(&dest.to_string_lossy()),
        ));
    }
    script
}

#[must_use]
pub fn write_managed_file_script(dir: &Path, staged: &Path, dest: &Path) -> String {
    format!(
        "set -e\n\
         /bin/mkdir -p {dir}\n\
         /usr/bin/install -m 0644 {staged} {dest}\n",
        dir = shell_quote(&dir.to_string_lossy()),
        staged = shell_quote(&staged.to_string_lossy()),
        dest = shell_quote(&dest.to_string_lossy()),
    )
}

#[must_use]
pub fn remove_managed_file_script(dest: &Path) -> String {
    format!(
        "set -e\n/bin/rm -f {}\n",
        shell_quote(&dest.to_string_lossy())
    )
}
