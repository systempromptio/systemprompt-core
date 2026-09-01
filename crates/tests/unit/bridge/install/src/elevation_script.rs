use std::path::Path;

use systemprompt_bridge::install::elevation_script::{
    applescript_escape, clear_policy_script, shell_quote, write_policy_script,
};

#[test]
fn applescript_escape_quotes_and_backslashes() {
    assert_eq!(applescript_escape(r#"echo "hi""#), r#"echo \"hi\""#);
    assert_eq!(
        applescript_escape(r"path\with\backslashes"),
        r"path\\with\\backslashes"
    );
    assert_eq!(applescript_escape("plain"), "plain");
}

#[test]
fn applescript_escape_multi_line_shell_scripts() {
    assert_eq!(
        applescript_escape("set -e\n/bin/rm -f '/tmp/x'\n"),
        r"set -e\n/bin/rm -f '/tmp/x'\n"
    );
    assert_eq!(applescript_escape("a\r\nb"), r"a\r\nb");
}

#[test]
fn shell_quote_wraps_and_escapes_single_quotes() {
    assert_eq!(shell_quote("plain"), "'plain'");
    assert_eq!(
        shell_quote("/Library/Application Support/ClaudeCode"),
        "'/Library/Application Support/ClaudeCode'"
    );
    assert_eq!(shell_quote("it's"), r"'it'\''s'");
}

#[test]
fn write_policy_script_installs_both_staged_files() {
    let script = write_policy_script(
        Path::new("/Library/Application Support/ClaudeCode"),
        Path::new("/tmp/stage/managed-mcp.json"),
        Path::new("/Library/Application Support/ClaudeCode/managed-mcp.json"),
        Path::new("/tmp/stage/managed-settings.json"),
        Path::new("/Library/Application Support/ClaudeCode/managed-settings.json"),
    );
    assert_eq!(
        script,
        "set -e\n\
         /bin/mkdir -p '/Library/Application Support/ClaudeCode'\n\
         /usr/bin/install -m 0644 '/tmp/stage/managed-mcp.json' '/Library/Application \
         Support/ClaudeCode/managed-mcp.json'\n\
         /usr/bin/install -m 0644 '/tmp/stage/managed-settings.json' '/Library/Application \
         Support/ClaudeCode/managed-settings.json'\n"
    );
}

#[test]
fn clear_policy_script_covers_each_combination() {
    assert_eq!(clear_policy_script(None, None), "set -e\n");
    assert_eq!(
        clear_policy_script(Some(Path::new("/etc/claude-code/managed-mcp.json")), None),
        "set -e\n/bin/rm -f '/etc/claude-code/managed-mcp.json'\n"
    );
    let both = clear_policy_script(
        Some(Path::new("/etc/claude-code/managed-mcp.json")),
        Some((
            Path::new("/tmp/stage/managed-settings.json"),
            Path::new("/etc/claude-code/managed-settings.json"),
        )),
    );
    assert_eq!(
        both,
        "set -e\n\
         /bin/rm -f '/etc/claude-code/managed-mcp.json'\n\
         /usr/bin/install -m 0644 '/tmp/stage/managed-settings.json' \
         '/etc/claude-code/managed-settings.json'\n"
    );
}

#[test]
fn write_policy_script_round_trips_through_applescript_escaping() {
    let script = write_policy_script(
        Path::new("/etc/claude-code"),
        Path::new("/tmp/a"),
        Path::new("/etc/claude-code/managed-mcp.json"),
        Path::new("/tmp/b"),
        Path::new("/etc/claude-code/managed-settings.json"),
    );
    let escaped = applescript_escape(&script);
    assert!(
        !escaped.contains('\n'),
        "an AppleScript literal cannot carry a raw newline: {escaped}"
    );
}

#[test]
fn write_managed_file_script_creates_the_dir_and_installs_world_readable() {
    use systemprompt_bridge::install::elevation_script::write_managed_file_script;
    let script = write_managed_file_script(
        Path::new("/Library/Application Support/opencode"),
        Path::new("/tmp/stage/opencode.json"),
        Path::new("/Library/Application Support/opencode/opencode.json"),
    );
    assert_eq!(
        script,
        "set -e\n\
         /bin/mkdir -p '/Library/Application Support/opencode'\n\
         /usr/bin/install -m 0644 '/tmp/stage/opencode.json' \
         '/Library/Application Support/opencode/opencode.json'\n"
    );
}

#[test]
fn remove_managed_file_script_removes_only_the_named_file() {
    use systemprompt_bridge::install::elevation_script::remove_managed_file_script;
    let script = remove_managed_file_script(Path::new("/etc/opencode/it's.json"));
    assert_eq!(script, "set -e\n/bin/rm -f '/etc/opencode/it'\\''s.json'\n");
}
