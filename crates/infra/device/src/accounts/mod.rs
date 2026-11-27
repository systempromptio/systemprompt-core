//! The systemprompt accounts of this install.
//!
//! [`Accounts`] is one cheap-clone handle over the instances this device knows
//! (at most one active), the sign-in flows (a browser device link through a
//! loopback callback, a pasted one-time code, a personal access token), the
//! sealed PATs, the session tokens minted from them and kept in memory only,
//! their renewal, and the trust of each instance's manifest key.
//!
//! No sign-in flow returns an error: each ends in a `SignInOutcome` the UI maps
//! to its copy. A sign-in only records a served manifest key as offered; trust
//! pins exactly the fingerprint the user was shown, and a different key served
//! later blocks manifest use until it is trusted. The snapshot the shell
//! renders from holds no secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod accounts;
mod browser;
mod browser_sign_in;
mod completion;
mod device_tokens;
mod error;
mod key_trust;
mod keys;
mod manage;
mod mint;
mod probe;
mod renewal;
pub mod schedule;
mod session;
mod signin;
mod slug;
mod snapshot;
mod state;
mod store;
mod summary;
mod sync_state;

pub use accounts::{Accounts, AccountsConfig, computer_name};
pub use browser::{Browser, BrowserError};
pub use browser_sign_in::{BrowserSignIn, SignInCancel};
pub use error::AccountsError;
pub use probe::{ProbeResult, ProbedInstance};
pub use session::Renewed;
pub use snapshot::{AccountsSnapshot, PendingKeyOffer};
pub use store::set_aside_corrupt_store;
pub use sync_state::DeviceEntry;
