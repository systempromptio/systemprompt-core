//! Device-authenticated consumer evidence: credentials, installation plans,
//! receipts and session bindings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod credentials;
mod plan;
mod receipts;
mod repository;
mod sessions;

pub use credentials::IssuedConsumerCredential;
pub use receipts::verify_readback;
pub use sessions::ConsumerSessionBinding;

use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::origin::ClientKind;

pub fn host_key(host: EvaluatorClient) -> &'static str {
    ClientKind::from(host).as_str()
}
