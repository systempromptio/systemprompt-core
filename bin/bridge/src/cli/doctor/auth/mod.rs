//! Doctor checks for stored credentials and gateway auth.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config_file;
mod gateway;
mod secrets;

pub use config_file::{
    check_cached_gateway, check_config_file, check_credential_source, check_install_record,
};
pub use gateway::{
    check_credential_store, check_gateway_reachable, check_hook_token_mint, check_mint_jwt,
    check_pinned_pubkey, check_whoami,
};
pub use secrets::{check_host_profile_secrets, check_loopback_secret};
