//! Scheduled jobs registered with the systemprompt scheduler via the
//! `inventory` crate: content prerender, page prerender, and copy-extension
//! -assets.

mod content_prerender;
mod copy_assets;
mod page_prerender;

pub use content_prerender::ContentPrerenderJob;
pub use copy_assets::execute_copy_extension_assets;
pub use page_prerender::PagePrerenderJob;
