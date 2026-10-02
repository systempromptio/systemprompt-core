use std::ffi::OsStr;
use std::process::Command;
use std::time::{Duration, Instant};

use systemprompt_bridge::user_alert::alert_command;

fn args(command: &Command) -> Vec<String> {
    command
        .get_args()
        .map(|arg| arg.to_str().expect("utf-8 argument").to_owned())
        .collect()
}

// The alert is raised from installer paths that have no window and no return
// channel, so the only contract it can break is holding the caller. The
// dialog program is handed to a shell that backgrounds it, so what the caller
// waits on is that shell. The launcher is proved here with a stand-in program
// that outlives the shell: a dialog is never raised by a test.
#[test]
fn the_launcher_returns_while_the_dialog_program_is_still_running() {
    let built = alert_command("Bridge needs attention", "Approve the managed profile.");
    assert_eq!(built.get_program(), OsStr::new("/bin/sh"));
    let built_args = args(&built);
    assert_eq!(built_args[0], "-c");
    let script = built_args[1].clone();

    let started = Instant::now();
    let status = Command::new("/bin/sh")
        .arg("-c")
        .arg(&script)
        .arg("sh")
        .args(["sleep", "30"])
        .status()
        .expect("launcher runs");
    let elapsed = started.elapsed();
    assert!(status.success(), "launcher failed: {status}");
    assert!(
        elapsed < Duration::from_secs(10),
        "the launcher must not wait on the dialog program; took {elapsed:?}"
    );
}

// The dialog program and its text reach the shell as positional parameters,
// never as script text, so nothing in the message is interpreted by it.
#[test]
fn the_message_is_a_positional_argument_not_shell_text() {
    let built = alert_command("Title", "body with $(touch /tmp/x) and `id`");
    let built_args = args(&built);
    assert!(!built_args[1].contains("touch"), "{built_args:?}");
    assert_eq!(built_args[2], "sh");
    assert!(
        built_args[3..]
            .iter()
            .any(|arg| arg.contains("$(touch /tmp/x)")),
        "{built_args:?}"
    );
}

// Quote characters are stripped rather than escaped because the AppleScript
// dialog text is quote-delimited; adversarial text must still produce a
// well-formed command.
#[test]
fn quote_characters_are_stripped_from_the_dialog_text() {
    let built = alert_command(
        "a \"quoted\" title with 'both' kinds",
        "body with \" and ' and a trailing backslash \\",
    );
    let dialog = args(&built)[3..].join(" ");
    assert!(
        dialog.contains("a quoted title with both kinds"),
        "{dialog}"
    );
    assert!(
        dialog.contains("body with  and  and a trailing backslash"),
        "{dialog}"
    );
}
