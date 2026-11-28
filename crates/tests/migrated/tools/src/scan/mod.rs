//! Splitting a command line into the commands it would run, for the shell
//! permission check: POSIX shells, PowerShell and cmd.

#[cfg(unix)]
mod oracle_tests;
mod posix_tests;
mod powershell_tests;
mod property_tests;

use systemprompt_engine::tools::{Dialect, split_commands};

pub(crate) fn heads(command: &str, dialect: Dialect) -> Vec<String> {
    split_commands(command, dialect)
        .iter()
        .filter_map(|segment| segment.head(dialect))
        .collect()
}

pub(crate) fn posix(command: &str) -> Vec<String> {
    heads(command, Dialect::Posix)
}

pub(crate) fn ps(command: &str) -> Vec<String> {
    heads(command, Dialect::PowerShell)
}

pub(crate) fn prefixes(command: &str, dialect: Dialect) -> Vec<String> {
    split_commands(command, dialect)
        .iter()
        .filter(|segment| segment.head(dialect).is_some())
        .map(|segment| segment.prefix(dialect))
        .collect()
}

pub(crate) fn paths(command: &str, dialect: Dialect) -> Vec<String> {
    split_commands(command, dialect)
        .iter()
        .flat_map(|segment| segment.path_words(dialect))
        .collect()
}

pub(crate) fn redirects(command: &str) -> Vec<Vec<String>> {
    split_commands(command, Dialect::Posix)
        .into_iter()
        .map(|segment| segment.redirects)
        .collect()
}

pub(crate) fn contains_all(found: &[String], wanted: &[&str]) -> bool {
    wanted
        .iter()
        .all(|want| found.iter().any(|have| have == want))
}
