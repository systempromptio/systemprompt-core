//! Stateless security services that don't fit elsewhere — currently the
//! [`ScannerDetector`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod scanner;

pub use scanner::ScannerDetector;
