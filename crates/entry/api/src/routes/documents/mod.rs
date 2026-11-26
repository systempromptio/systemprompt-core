//! The `files-http` host handles the API reads from the kernel state.
//!
//! The document, folder, upload, public-link, issue and bridge Drive handlers
//! are the `files-http` member's; the internal link route is the
//! `documents` member's, reached through its sealed `DocumentsKernel`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::router::FilesHostHandles;
use systemprompt_manifest::Config;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

use crate::kernel::resolve_client_ip;

pub fn files_host_handles(ctx: &AppContext) -> Result<FilesHostHandles, MissingDependency> {
    let trusted = Arc::new(
        ctx.kernel()
            .state()
            .get::<Arc<Config>>()?
            .trusted_proxies
            .clone(),
    );
    Ok(FilesHostHandles {
        users: ctx.user_provider(),
        client_ip: Arc::new(move |headers, connect_info| {
            resolve_client_ip(headers, connect_info, &trusted)
        }),
    })
}
