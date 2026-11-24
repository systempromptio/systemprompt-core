//! Locating the `systemprompt-bridge` binary a subprocess test spawns.
//!
//! `SP_BRIDGE_BIN` names it explicitly (the coverage recipe points it at an
//! instrumented build). Otherwise the binary is the one in the cargo target
//! directory this test executable was built into: the test runs from
//! `<target>/<profile>/deps/<test>-<hash>`, so two parents up is the profile
//! directory a `cargo build --bin systemprompt-bridge` in the same workspace
//! writes to. Deriving it from the running executable follows
//! `CARGO_TARGET_DIR`, `--target <triple>` and custom profiles without
//! reading any of them.

use std::path::PathBuf;

const BINARY: &str = "systemprompt-bridge";

pub(crate) fn candidate() -> PathBuf {
    if let Some(explicit) = std::env::var_os("SP_BRIDGE_BIN") {
        return PathBuf::from(explicit);
    }
    let exe = std::env::current_exe().expect("the running test executable has a path");
    let deps = exe
        .parent()
        .expect("the test executable sits in a target deps directory");
    let profile = if deps.file_name().is_some_and(|name| name == "deps") {
        deps.parent()
            .expect("a deps directory sits in a target profile directory")
    } else {
        deps
    };
    profile.join(format!("{BINARY}{}", std::env::consts::EXE_SUFFIX))
}
