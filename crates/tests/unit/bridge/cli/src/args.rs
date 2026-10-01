use systemprompt_bridge::cli::args::{
    UsageError, check_flags, has_flag, parse_multi_flag, parse_opt_flag,
};
use systemprompt_bridge::cli::flags::command_flags;

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn parse_opt_flag_finds_value() {
    let a = args(&["bin", "sub", "--host", "codex-cli"]);
    assert_eq!(parse_opt_flag(&a, "--host"), Some("codex-cli".to_owned()));
}

#[test]
fn parse_opt_flag_absent_returns_none() {
    let a = args(&["bin", "sub", "--other", "value"]);
    assert_eq!(parse_opt_flag(&a, "--host"), None);
}

#[test]
fn parse_opt_flag_last_arg_without_value_returns_none() {
    let a = args(&["bin", "sub", "--host"]);
    assert_eq!(parse_opt_flag(&a, "--host"), None);
}

#[test]
fn parse_opt_flag_ignores_index_below_two() {
    let a = args(&["bin", "--host", "ignored"]);
    assert_eq!(parse_opt_flag(&a, "--host"), None);
}

#[test]
fn parse_opt_flag_returns_first_match_when_repeated() {
    let a = args(&["bin", "sub", "--host", "first", "--host", "second"]);
    assert_eq!(parse_opt_flag(&a, "--host"), Some("first".to_owned()));
}

#[test]
fn has_flag_true_when_present() {
    let a = args(&["bin", "sub", "--apply"]);
    assert!(has_flag(&a, "--apply"));
}

#[test]
fn has_flag_false_when_absent() {
    let a = args(&["bin", "sub", "--other"]);
    assert!(!has_flag(&a, "--apply"));
}

#[test]
fn has_flag_false_when_only_at_index_one() {
    let a = args(&["bin", "--apply"]);
    assert!(!has_flag(&a, "--apply"));
}

#[test]
fn parse_multi_flag_collects_every_occurrence() {
    let a = args(&["bin", "sub", "--host", "opencode", "--host", "codex-cli"]);
    assert_eq!(
        parse_multi_flag(&a, "--host"),
        vec!["opencode".to_owned(), "codex-cli".to_owned()]
    );
}

#[test]
fn parse_multi_flag_splits_comma_separated_values() {
    let a = args(&["bin", "sub", "--host", "claude-code,opencode"]);
    assert_eq!(
        parse_multi_flag(&a, "--host"),
        vec!["claude-code".to_owned(), "opencode".to_owned()]
    );
}

#[test]
fn parse_multi_flag_drops_blanks_and_duplicates() {
    let a = args(&[
        "bin",
        "sub",
        "--host",
        "opencode, ,opencode,",
        "--host",
        " opencode ",
    ]);
    assert_eq!(parse_multi_flag(&a, "--host"), vec!["opencode".to_owned()]);
}

#[test]
fn parse_multi_flag_absent_is_empty() {
    let a = args(&["bin", "sub", "--apply"]);
    assert!(parse_multi_flag(&a, "--host").is_empty());
}

#[test]
fn parse_multi_flag_ignores_index_below_two() {
    let a = args(&["bin", "--host", "ignored"]);
    assert!(parse_multi_flag(&a, "--host").is_empty());
}

#[test]
fn parse_multi_flag_last_arg_without_value_is_empty() {
    let a = args(&["bin", "sub", "--host"]);
    assert!(parse_multi_flag(&a, "--host").is_empty());
}

fn check(items: &[&str]) -> Result<(), UsageError> {
    let a = args(items);
    let spec = command_flags(a.get(1).map(String::as_str)).expect("a known command");
    check_flags(&a, &spec)
}

#[test]
fn a_flag_where_a_value_belongs_is_refused_rather_than_consumed() {
    assert_eq!(
        check(&["bin", "install", "--pubkey", "--apply"]),
        Err(UsageError::FlagAsValue {
            flag: "--pubkey".to_owned(),
            found: "--apply".to_owned(),
        })
    );
}

#[test]
fn a_misspelt_security_flag_is_refused() {
    assert_eq!(
        check(&["bin", "sync", "--allow-unsigend"]),
        Err(UsageError::UnknownFlag("--allow-unsigend".to_owned()))
    );
    assert_eq!(
        check(&["bin", "sync", "--force-replay"]),
        Ok(()),
        "the negative control: the real flag is accepted"
    );
}

#[test]
fn a_trailing_value_flag_without_a_value_is_refused() {
    assert_eq!(
        check(&["bin", "install", "--gateway"]),
        Err(UsageError::MissingValue("--gateway".to_owned()))
    );
}

#[test]
fn positionals_are_counted_per_command() {
    assert_eq!(check(&["bin", "login", "sp-live-token"]), Ok(()));
    assert_eq!(
        check(&["bin", "login", "sp-live-token", "extra"]),
        Err(UsageError::UnexpectedArgument("extra".to_owned()))
    );
    assert_eq!(
        check(&["bin", "status", "extra"]),
        Err(UsageError::UnexpectedArgument("extra".to_owned()))
    );
}

#[test]
fn inline_values_are_accepted_only_where_the_command_reads_them() {
    assert_eq!(
        check(&["bin", "credential-helper", "--host=codex-cli"]),
        Ok(())
    );
    assert_eq!(
        check(&["bin", "install", "--gateway=https://gw.example"]),
        Err(UsageError::UnknownFlag(
            "--gateway=https://gw.example".to_owned()
        ))
    );
}

#[test]
fn every_flag_a_command_documents_is_accepted() {
    assert_eq!(
        check(&[
            "bin",
            "install",
            "--gateway",
            "https://gw.example",
            "--pubkey",
            "a2V5",
            "--apply",
            "--apply-schedule",
            "--host",
            "opencode",
            "--egress-allowed-hosts",
            "loopback",
        ]),
        Ok(())
    );
    assert_eq!(check(&["bin", "update", "--check", "-y"]), Ok(()));
    assert_eq!(
        check(&["bin", "uninstall", "--host", "hermes", "--purge"]),
        Ok(())
    );
    assert_eq!(check(&["bin", "oauth-client", "rotate"]), Ok(()));
}

#[test]
fn an_unknown_command_has_no_flag_table() {
    assert!(command_flags(Some("no-such-command")).is_none());
    assert!(command_flags(None).is_none());
}
