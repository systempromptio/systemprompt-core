//! Config-loader input/output types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::path::PathBuf;

use systemprompt_models::services::ServicesConfig;

pub(super) struct IncludeResolveCtx<'a> {
    pub visited: &'a mut HashSet<PathBuf>,
    pub merged: &'a mut ServicesConfig,
    pub chain: Vec<PathBuf>,
}
