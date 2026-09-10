use std::process::ExitCode;

use systemprompt_bridge::auth::ChainError;
use systemprompt_bridge::auth::providers::AuthFailedSource;

// Why: `ExitCode` has no `PartialEq`, so the observable is its `Debug` shape.
fn code(actual: ExitCode) -> String {
    format!("{actual:?}")
}

#[test]
fn every_chain_failure_maps_to_a_distinct_operator_facing_outcome() {
    let cases: Vec<(ChainError, ExitCode, &str)> = vec![
        (
            ChainError::Providers {
                failures: vec!["pat: 401".to_owned(), "mtls: no cert".to_owned()],
                terminal: true,
            },
            ExitCode::FAILURE,
            "credential providers failed: pat: 401; mtls: no cert",
        ),
        (
            ChainError::Cache(std::io::Error::other("cache.json is a directory")),
            ExitCode::FAILURE,
            "credential cache: cache.json is a directory",
        ),
    ];
    for (error, expected_code, expected_message) in cases {
        let (actual_code, message) = error.exit_report();
        assert_eq!(code(actual_code), code(expected_code), "{message}");
        assert_eq!(
            message, expected_message,
            "a terminal chain failure reports its own Display text unchanged"
        );
        assert_eq!(
            message,
            error.to_string(),
            "the operator sees the same text whether it is printed or exited on"
        );
    }
}

#[test]
fn a_transient_failure_on_the_preferred_provider_exits_10_so_a_retry_is_distinguishable() {
    let error = ChainError::PreferredTransient {
        provider: "mtls",
        source: AuthFailedSource::SignInRequired,
    };
    let (actual, message) = error.exit_report();
    assert_eq!(code(actual), code(ExitCode::from(10)));
    assert!(
        message.contains("transient auth failure on preferred provider mtls"),
        "the caller must be able to tell a retryable failure from a dead credential: {message}"
    );
    assert!(
        message.contains("sign in"),
        "the underlying reason travels with the exit report: {message}"
    );
}

#[test]
fn an_empty_chain_exits_5_and_names_the_command_that_fixes_it() {
    let (actual, message) = ChainError::NoneSucceeded.exit_report();
    assert_eq!(code(actual), code(ExitCode::from(5)));
    assert!(
        message.contains("login <sp-live-...>"),
        "a signed-out install is told what to run, not just that it failed: {message}"
    );
    assert!(
        message.contains(systemprompt_bridge::brand::brand().binary_name),
        "the fix names this binary: {message}"
    );
}

#[test]
fn the_exit_codes_of_the_four_outcomes_are_not_all_the_same() {
    // Why: the negative control. A mapping that collapsed every variant onto
    // FAILURE would satisfy each test above that only checks the message.
    let codes: Vec<String> = vec![
        ChainError::Providers {
            failures: Vec::new(),
            terminal: true,
        },
        ChainError::PreferredTransient {
            provider: "pat",
            source: AuthFailedSource::SignInRequired,
        },
        ChainError::NoneSucceeded,
    ]
    .into_iter()
    .map(|error| code(error.exit_report().0))
    .collect();
    let mut unique = codes.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 3, "codes: {codes:?}");
}
