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
