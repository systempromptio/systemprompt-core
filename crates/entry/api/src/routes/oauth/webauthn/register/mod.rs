mod finish;
mod start;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RegisterError {
    pub error: String,
    pub error_description: String,
}

pub use finish::finish_register;
pub use start::start_register;
