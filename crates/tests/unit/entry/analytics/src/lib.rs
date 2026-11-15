//! Tests for `systemprompt-analytics-entry`'s commands: the `analytics`
//! group (dispatch, seeded reports, table and JSON output, CSV export, the
//! time-range parsers) and the `core content popular` leaf, plus their
//! registration with the CLI kernel.

#[cfg(test)]
mod boot_ctx;
#[cfg(test)]
mod cli;
