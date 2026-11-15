//! The generator's jobs: the content and page prerender jobs the generator
//! member contributes, and the copy-extension-assets step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content_prerender;
mod copy_assets;
mod page_prerender;

pub use content_prerender::ContentPrerenderJob;
pub use copy_assets::{copy_asset, execute_copy_extension_assets};
pub use page_prerender::PagePrerenderJob;
