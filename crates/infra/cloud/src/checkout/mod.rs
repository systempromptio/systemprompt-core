//! Cloud checkout flow: drive a subscription purchase and wait for the tenant
//! to finish provisioning.
//!
//! Re-exports the callback-flow entry point [`run_checkout_callback_flow`] and
//! the [`wait_for_provisioning`] poller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod provisioning;

pub use client::{CheckoutCallbackResult, CheckoutTemplates, run_checkout_callback_flow};
pub use provisioning::wait_for_provisioning;
