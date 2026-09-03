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
pub mod webauthn_challenge;

pub use bridge_host_prefs::BridgeHostPrefsRepository;
pub use bridge_session::{BridgeSessionRepository, BridgeSessionRow, UpsertBridgeSession};
pub use client::{
    ClientRepository, ClientSummary, ClientUsageSummary, CreateClientParams, UpdateClientParams,
};
pub use exchange_code::CreateExchangeCodeParams;
pub use oauth::{
    AuthCodeParams, AuthCodeValidationResult, JtiRevocationCache, MintAuthCodeParams,
    OAuthRepository, RefreshTokenParams, StateBindingParams, StateBindingRow,
};
pub use setup_token::{
    CreateSetupTokenParams, SetupTokenPurpose, SetupTokenRecord, TokenValidationResult,
};
pub use webauthn::{WebAuthnCredential, WebAuthnCredentialParams};
pub use webauthn_challenge::{
    ConsumedChallenge, LinkChallengeReservation, ReserveLinkChallengeParams, StoreChallengeParams,
    WebAuthnChallengeKind,
};

use crate::error::OauthResult;
use systemprompt_database::DbPool;

/// Bundle of the OAuth-domain repositories, constructed once at a composition
/// root and cloned by consumers.
#[derive(Debug, Clone)]
pub struct OAuthRepositories {
    pub oauth: OAuthRepository,
    pub bridge_host_prefs: BridgeHostPrefsRepository,
    pub bridge_sessions: BridgeSessionRepository,
}

impl OAuthRepositories {
    pub fn new(db: &DbPool) -> OauthResult<Self> {
        Ok(Self {
            oauth: OAuthRepository::new(db)?,
            bridge_host_prefs: BridgeHostPrefsRepository::new(db)?,
            bridge_sessions: BridgeSessionRepository::new(db)?,
        })
    }
}
