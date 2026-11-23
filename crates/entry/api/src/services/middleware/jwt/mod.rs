//! The API's request-context extractor over the kernel's request
//! authenticator.
//!
//! [`JwtContextExtractor`] adapts the kernel-seeded
//! `DynRequestAuthenticator` to the context middleware's
//! [`ContextExtractor`](super::context::ContextExtractor): every
//! verification it performs is the authenticator's. It is built from the
//! kernel state (`JwtContextExtractor::from_kernel`), never from the app
//! context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod context;

pub use context::JwtContextExtractor;
pub use systemprompt_models::auth::JwtUserContext;
