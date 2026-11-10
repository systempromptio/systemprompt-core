//! `admin setup --db-password-stdin`: the password is read from the first
//! line of stdin, never from argv.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::setup::read_password_line;
use systemprompt_cli::args::Cli;

#[test]
fn reads_the_first_line_without_its_terminator() {
    let password = read_password_line(&b"p@ss w#rd\r\nignored\n"[..]).unwrap();
    assert_eq!(password, "p@ss w#rd");
}

#[test]
fn an_empty_line_is_refused() {
    let error = read_password_line(&b"\n"[..]).unwrap_err();
    assert!(error.to_string().contains("empty password"), "{error:#}");
}

#[test]
fn stdin_and_argv_passwords_conflict() {
    let parsed = Cli::try_parse_from([
        "systemprompt",
        "admin",
        "setup",
        "--db-password-stdin",
        "--db-password",
        "x",
    ]);
    assert!(parsed.is_err());
    Cli::try_parse_from(["systemprompt", "admin", "setup", "--db-password-stdin"])
        .expect("the stdin switch takes no value");
}
