//! Prints the member ids of this build's reference composition, one per line,
//! in composition order. CI compares them for every booted composition with
//! `scripts/composition-members.txt`.
//!
//! Run with: `cargo run -p systemprompt --example members --no-default-features
//! --features core,runtime`

use std::io::Write;

fn main() -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    for id in systemprompt::reference().member_ids() {
        writeln!(out, "{id}")?;
    }
    Ok(())
}
