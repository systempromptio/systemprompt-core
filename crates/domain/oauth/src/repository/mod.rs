//! Persistence repositories backing the OAuth domain (clients, codes, tokens,
//! `WebAuthn` credentials).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bridge_host_prefs;
pub mod bridge_session;
pub mod client;
pub mod exchange_code;
pub mod oauth;
pub mod setup_token;
pub mod webauthn;

pub use bridge_host_prefs::BridgeHostPrefsRepository;
pub use bridge_session::{BridgeSessionRepository, BridgeSessionRow, UpsertBridgeSession};
pub use client::{
    ClientRepository, ClientSummary, ClientUsageSummary, CreateClientParams, UpdateClientParams,
};
pub use exchange_code::CreateExchangeCodeParams;
pub use oauth::{
    AuthCodeParams, AuthCodeValidationResult, JtiRevocationCache, OAuthRepository,
    RefreshTokenParams, StateBindingParams, StateBindingRow,
};
pub use setup_token::{
    CreateSetupTokenParams, SetupTokenPurpose, SetupTokenRecord, TokenValidationResult,
};
pub use webauthn::{WebAuthnCredential, WebAuthnCredentialParams};
