//! Minting a session token from the account's sealed PAT.
//!
//! The instance refusing the PAT signs the account out (the PAT is forgotten
//! locally); a ciphertext the OS cannot open right now is kept and reported as
//! `CredentialError::Insecure`; one this computer will never open again signs
//! the account out. A session id the instance no longer recognises is dropped
//! and the mint retried without it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::client::{ClientError, CredentialError, JwtSession, SpClient};
use crate::protocol::Pat;
use crate::protocol::ids::AccountId;
use crate::secure::SealedString;
use systemprompt_identifiers::SessionId;

use crate::accounts::handle::Accounts;
use crate::accounts::state::State;

type MintInputs = (Option<(String, SealedString)>, Option<SessionId>);

enum Opened {
    Pat(Pat),
    Later,
    Never,
}

impl Accounts {
    pub(crate) async fn mint(&self, account: &AccountId) -> Result<JwtSession, CredentialError> {
        let (found, session_id) = mint_inputs(&self.inner.state.lock(), account);
        let (url, credential) = found.ok_or(CredentialError::SignedOut)?;
        let pat = match self.open_pat(&credential) {
            Opened::Pat(pat) => pat,
            Opened::Later => return Err(CredentialError::Insecure),
            Opened::Never => {
                tracing::warn!(
                    account = %account,
                    "the stored personal access token will not open here; signing out"
                );
                self.sign_out_holding(account, &credential);
                return Err(CredentialError::SignedOut);
            },
        };
        let client = SpClient::new(&url, self.inner.http.clone())
            .map_err(|error| CredentialError::Mint(Box::new(error)))?;
        let minted = match client.mint_session(&pat, session_id.as_ref()).await {
            Err(ClientError::SessionMismatch { .. }) if session_id.is_some() => {
                client.mint_session(&pat, None).await
            },
            other => other,
        };
        drop(pat);
        match minted {
            Ok(session) => self.install_session(account, session, credential),
            Err(error) if error.is_unauthorized() => {
                tracing::warn!(
                    account = %account,
                    error = %error,
                    "the instance refused the personal access token; signing out"
                );
                self.sign_out_holding(account, &credential);
                Err(CredentialError::Refused)
            },
            Err(error) => {
                tracing::debug!(
                    account = %account,
                    error = %error,
                    "minting a session token failed"
                );
                Err(CredentialError::Mint(Box::new(error)))
            },
        }
    }

    fn open_pat(&self, credential: &SealedString) -> Opened {
        match credential.open(&*self.inner.sealer) {
            Ok(opened) => Pat::new(opened.as_str()).map_or(Opened::Never, Opened::Pat),
            Err(error) if error.keeps_ciphertext() => {
                tracing::warn!(
                    error = %error,
                    "the OS secure store cannot open the personal access token right now"
                );
                Opened::Later
            },
            Err(error) => {
                tracing::warn!(error = %error, "the personal access token ciphertext is refused");
                Opened::Never
            },
        }
    }

    fn sign_out_holding(&self, account: &AccountId, credential: &SealedString) {
        let holding =
            self.inner.state.lock().current_credential(account).as_ref() == Some(credential);
        if holding && let Err(error) = self.sign_out(account) {
            tracing::warn!(account = %account, error = %error, "signing the account out failed");
        }
    }
}

fn mint_inputs(state: &State, account: &AccountId) -> MintInputs {
    let found = state
        .store
        .find(account)
        .and_then(|record| Some((record.url.clone(), state.credential(record)?)));
    let session_id = state
        .sessions
        .get(account)
        .and_then(|slot| slot.held.as_ref())
        .and_then(|held| held.session.session_id.clone());
    (found, session_id)
}
