//! The `update` command's argument parsing and the arms it reaches before a
//! credential or a gateway is available.

use std::process::ExitCode;

use systemprompt_bridge::cli::update::test_api::{confirm, progress_reporter};
use systemprompt_bridge::cli::update::{cmd_update, parse};
use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::update::DownloadProgress;
use tempfile::TempDir;

fn argv(parts: &[&str]) -> Vec<String> {
    let mut v = vec!["systemprompt-bridge".to_owned(), "update".to_owned()];
    v.extend(parts.iter().map(|s| (*s).to_owned()));
    v
}

fn in_sandbox<R>(f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("home");
    let root = home.path().display().to_string();
    let out = temp_env::with_vars(
        [
            ("HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
            ("XDG_STATE_HOME", Some(format!("{root}/.state"))),
            ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
            ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
            ("SP_BRIDGE_PAT", None),
            ("SP_BRIDGE_CONFIG", None),
            ("SUDO_USER", None),
        ],
        f,
    );
    drop(home);
    out
}

fn code(c: ExitCode) -> String {
    format!("{c:?}")
}

#[test]
fn no_flags_parses_to_an_interactive_full_update() {
    let args = parse(&argv(&[])).expect("no flags is valid");
    assert!(!args.check_only);
    assert!(!args.assume_yes);
}

#[test]
fn the_check_flag_asks_only_whether_an_update_exists() {
    let args = parse(&argv(&["--check"])).expect("--check is valid");
    assert!(args.check_only);
    assert!(!args.assume_yes);
}

#[test]
fn both_spellings_of_the_yes_flag_are_accepted() {
    assert!(parse(&argv(&["--yes"])).expect("--yes").assume_yes);
    assert!(parse(&argv(&["-y"])).expect("-y").assume_yes);
}

#[test]
fn the_flags_combine_rather_than_overriding_one_another() {
    let args = parse(&argv(&["--check", "--yes"])).expect("both flags");
    assert!(args.check_only);
    assert!(args.assume_yes);
}

#[test]
fn a_repeated_flag_is_accepted_rather_than_rejected() {
    let args = parse(&argv(&["--check", "--check"])).expect("repeats are harmless");
    assert!(args.check_only);
}

#[test]
fn an_unknown_flag_is_rejected_and_the_error_names_it() {
    let err = parse(&argv(&["--frobnicate"])).expect_err("unknown flag");
    let rendered = err.to_string();
    assert!(rendered.contains("--frobnicate"), "got {rendered}");
    assert!(rendered.contains("update"), "got {rendered}");
}

#[test]
fn the_first_two_argv_entries_are_the_binary_and_the_command_not_flags() {
    let args = parse(&["--check".to_owned(), "--yes".to_owned()])
        .expect("the first two entries are skipped, whatever they are");
    assert!(
        !args.check_only && !args.assume_yes,
        "argv[0] and argv[1] must not be read as flags"
    );
}

#[test]
fn an_unparseable_command_line_exits_sixty_four_without_contacting_a_gateway() {
    let exit = in_sandbox(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("attach context");
        cmd_update(&ctx, &argv(&["--nope"]))
    });
    assert_eq!(
        code(exit),
        code(ExitCode::from(64)),
        "a usage error is exit 64, distinct from an update failure"
    );
}

#[test]
fn with_no_credential_stored_the_command_exits_five_and_says_to_log_in() {
    let exit = in_sandbox(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("attach context");
        cmd_update(&ctx, &argv(&["--check"]))
    });
    assert_eq!(
        code(exit),
        code(ExitCode::from(5)),
        "no credential is its own exit code, not a generic failure"
    );
}

#[test]
fn the_no_credential_exit_is_distinct_from_the_usage_and_failure_codes() {
    let no_credential = in_sandbox(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("attach");
        cmd_update(&ctx, &argv(&[]))
    });
    let usage = in_sandbox(|| {
        let ctx = BridgeContext::start(ProxyMode::Attach).expect("attach");
        cmd_update(&ctx, &argv(&["--bad"]))
    });

    assert_ne!(
        code(no_credential),
        code(usage),
        "a cron probe must be able to tell a missing login from a bad flag"
    );
    assert_eq!(code(no_credential), code(ExitCode::from(5)));
    assert_eq!(code(usage), code(ExitCode::from(64)));
}

#[test]
fn the_progress_reporter_survives_every_ratio_including_a_zero_byte_download() {
    let report = progress_reporter();

    for (received, total) in [(0, 0), (0, 100), (50, 100), (100, 100), (200, 100)] {
        report(DownloadProgress { received, total });
    }

    let fractions: Vec<f64> = [(0u64, 0u64), (50, 100), (200, 100)]
        .into_iter()
        .map(|(received, total)| DownloadProgress { received, total }.fraction())
        .collect();
    assert_eq!(
        fractions,
        vec![0.0, 0.5, 1.0],
        "the reporter multiplies fraction() by 100 and casts to u8, so it must stay clamped"
    );
}

#[test]
fn without_a_terminal_the_install_prompt_declines_rather_than_blocking_on_stdin() {
    assert!(
        !confirm("9.9.9"),
        "an unattended run must decline rather than wait for an answer nobody will give"
    );
}
