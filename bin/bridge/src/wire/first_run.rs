//! First-run (setup) progress as the webview receives it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct FirstRunHostPayload<'a> {
    pub host_id: &'a str,
    pub display_name: &'a str,
    pub status: &'static str,
    pub error: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct FirstRunPayload<'a> {
    pub active: bool,
    pub done: bool,
    pub phase: &'static str,
    pub sync: &'static str,
    pub error: Option<&'a str>,
    pub hosts: Vec<FirstRunHostPayload<'a>>,
}
