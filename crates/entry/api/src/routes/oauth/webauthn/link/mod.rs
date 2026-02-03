mod finish;
mod page;
mod start;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LinkError {
    pub error: String,
    pub error_description: String,
}

pub use finish::finish_link;
pub use page::link_passkey_page;
pub use start::start_link;
