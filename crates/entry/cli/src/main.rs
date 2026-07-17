//! `systemprompt` binary entry point.
//!
//! Thin wrapper around the library entry [`systemprompt_cli::run`] so the cli
//! ships both as a library (for embedders) and as a runnable binary (so
//! coverage / integration harnesses can invoke it via `cargo_bin`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(systemprompt_cli::run())
}
