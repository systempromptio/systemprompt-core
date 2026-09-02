use systemprompt_bridge::cli::update::{Args, parse};

fn argv(rest: &[&str]) -> Vec<String> {
    let mut v = vec!["bridge".to_owned(), "update".to_owned()];
    v.extend(rest.iter().map(|s| (*s).to_owned()));
    v
}

#[test]
fn bare_update_installs_interactively() {
    assert!(matches!(
        parse(&argv(&[])),
        Ok(Args {
            check_only: false,
            assume_yes: false
        })
    ));
}

#[test]
fn flags_parse() {
    assert!(matches!(
        parse(&argv(&["--check"])),
        Ok(Args {
            check_only: true,
            ..
        })
    ));
    assert!(matches!(
        parse(&argv(&["-y"])),
        Ok(Args {
            assume_yes: true,
            ..
        })
    ));
}

#[test]
fn unknown_flag_is_rejected() {
    assert!(parse(&argv(&["--force"])).is_err());
}

#[test]
fn both_flags_combine() {
    assert!(matches!(
        parse(&argv(&["--check", "--yes"])),
        Ok(Args {
            check_only: true,
            assume_yes: true
        })
    ));
}

#[test]
fn repeated_flags_are_idempotent() {
    assert!(matches!(
        parse(&argv(&["-y", "--yes", "-y"])),
        Ok(Args {
            assume_yes: true,
            check_only: false
        })
    ));
}

#[test]
fn the_first_two_argv_entries_are_never_read_as_flags() {
    let argv = vec!["--check".to_owned(), "--yes".to_owned()];
    assert!(matches!(
        parse(&argv),
        Ok(Args {
            check_only: false,
            assume_yes: false
        })
    ));
}

#[test]
fn unknown_flag_error_names_the_flag() {
    let err = parse(&argv(&["--check", "--force"])).expect_err("unknown flag");
    assert_eq!(err.to_string(), "unknown flag for `update`: --force");
}

#[test]
fn a_positional_argument_is_rejected() {
    let err = parse(&argv(&["1.2.3"])).expect_err("positional argument");
    assert_eq!(err.to_string(), "unknown flag for `update`: 1.2.3");
}
