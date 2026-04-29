pub mod hop_by_hop;
pub mod request;
pub mod response;

pub use hop_by_hop::is_hop_by_hop;
pub use request::{Request, parse, parse_from_read};
pub use response::{ResponseBuilder, write_chunked};

#[derive(Debug, thiserror::Error)]
pub enum HttpLocalError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing method in request line")]
    MissingMethod,
    #[error("missing target in request line")]
    MissingTarget,
    #[error("headers too large")]
    HeadersTooLarge,
    #[error("body too large: {0} bytes")]
    BodyTooLarge(usize),
    #[error("chunked body too large")]
    ChunkedBodyTooLarge,
    #[error("parse content-length: {0}")]
    ParseContentLength(#[source] std::num::ParseIntError),
    #[error("parse chunk size: {0}")]
    ParseChunkSize(#[source] std::num::ParseIntError),
}

pub type Result<T> = std::result::Result<T, HttpLocalError>;
