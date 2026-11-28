use std::path::Path;

use systemprompt_machine::elevation::{
    ElevatedCommand, Elevation, ElevationError, authorization_script, current, manual_command,
};

use crate::support::{program, program_text};

fn command(arguments: &[&str], reason: &str) -> ElevatedCommand {
    ElevatedCommand::new(
        &program(),
        arguments.iter().map(|a| (*a).to_owned()).collect(),
        reason,
    )
    .expect("a valid elevated command")
}

#[test]
fn relative_program_is_refused() {
    let outcome = ElevatedCommand::new(Path::new("example"), Vec::new(), "apply the policy");
    assert!(matches!(outcome, Err(ElevationError::Program { .. })));
}

#[test]
fn request_without_a_reason_is_refused() {
    for reason in ["", "   ", "two\nlines"] {
        let outcome = ElevatedCommand::new(&program(), Vec::new(), reason);
        assert!(
            matches!(outcome, Err(ElevationError::Reason)),
            "reason {reason:?} must be refused"
        );
    }
}

#[test]
fn argument_with_a_control_character_names_its_position() {
    let outcome = ElevatedCommand::new(
        &program(),
        vec!["apply".to_owned(), "bad\u{0}value".to_owned()],
        "apply the policy",
    );
    assert!(matches!(
        outcome,
        Err(ElevationError::Argument { index: 1 })
    ));
}

#[test]
fn manual_command_quotes_every_word_for_a_shell() {
    let built = command(&["--name", "it's here"], "apply the policy");
    assert_eq!(
        manual_command(&built),
        format!(r"sudo -- '{}' '--name' 'it'\''s here'", program_text())
    );
}

#[test]
fn authorization_script_escapes_the_command_and_carries_the_reason() {
    let built = command(
        &[r#"say "hi""#, r"back\slash"],
        r#"Apply the "managed" policy"#,
    );
    let script = authorization_script(&built);

    assert!(script.starts_with("do shell script \""));
    assert!(
        script.ends_with(
            r#"with prompt "Apply the \"managed\" policy" with administrator privileges"#
        )
    );
    assert!(
        script.contains(r#"'say \"hi\"'"#),
        "quotes escaped: {script}"
    );
    assert!(
        script.contains(r"'back\\slash'"),
        "backslash escaped: {script}"
    );
}

#[test]
fn elevation_of_the_test_process_is_readable() {
    let elevation = current().expect("the process can read its own elevation");
    assert!(matches!(
        elevation,
        Elevation::Elevated | Elevation::Standard
    ));
}
