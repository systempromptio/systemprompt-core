//! The one place a test is allowed to decide it cannot run.
//!
//! A test tier that returns early when a prerequisite is missing reports the
//! same green as a tier that ran. That is only tolerable on a developer
//! machine with no Postgres; in CI it is the failure mode the tier exists to
//! catch, so `CI=true` turns every skip into a panic carrying the missing
//! prerequisite's name.
//!
//! Locally each skip prints one `SKIP <what> -- <reason>` line to stderr, so a
//! run that covered nothing is visible in the log rather than inferred from a
//! suspiciously fast green.

pub fn ci() -> bool {
    std::env::var_os("CI").is_some_and(|v| !v.is_empty() && v != "0" && v != "false")
}

// Why: returns `false` so a caller written as `gate() || return` reads as the
// skip path; under CI it never returns at all.
pub fn skip_or_panic(what: &str, reason: &str) -> bool {
    assert!(
        !ci(),
        "missing test prerequisite in CI: {what} -- {reason}. A skipped test in CI is a failed \
         test: provision the prerequisite or delete the test."
    );
    eprintln!("SKIP {what} -- {reason}");
    false
}
