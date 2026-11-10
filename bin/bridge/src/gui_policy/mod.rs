//! Security decisions the desktop GUI makes, kept free of any webview or
//! windowing dependency so every target compiles and tests them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod trust;
pub mod webview;
