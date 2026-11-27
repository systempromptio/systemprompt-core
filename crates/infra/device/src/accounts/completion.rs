//! The completion every sign-in flow shares once it holds a PAT.
//!
//! It mints a session, asks who the user is, fetches the manifest key and
//! records it as offered (a sign-in never trusts a key), summarises the first
//! manifest verified against that key, and stores the account, active only when
//! no account is. A user other than the one the account last held forgets the
//! previous user's sync state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::client::{JwtSession, ManifestFetch, ManifestKey, SpClient};
use crate::protocol::Pat;
use crate::protocol::ids::{AccountId, InstanceKey};
use crate::secure::SealedString;
use crate::view::{InstanceSummary, SignInFailure, SignInOutcome};
use chrono::Utc;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::gateway::WhoamiResponse;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::accounts::error::AccountsError;
use crate::accounts::handle::Accounts;
use crate::accounts::keys;
use crate::accounts::probe::label_for;
use crate::accounts::signin::{Reached, until_cancelled};
use crate::accounts::slug::unique_slug;
use crate::accounts::store::{AccountRecord, StoreFile};
use crate::accounts::summary::summarize;

struct FirstContact {
    session: JwtSession,
    whoami: Option<WhoamiResponse>,
    key: Option<ManifestKey>,
    summary: Option<InstanceSummary>,
}

struct Arrival {
    base: String,
    label: String,
    user: Option<String>,
    user_id: Option<UserId>,
    org: Option<String>,
    pat: Option<SealedString>,
    key: Option<ManifestKey>,
}

impl Accounts {
    pub(crate) async fn complete(
        &self,
        reached: &Reached,
        pat: Pat,
        keep: bool,
        cancel: Option<&CancellationToken>,
    ) -> SignInOutcome {
        let contacted = match cancel {
            Some(cancel) => until_cancelled(cancel, first_contact(&reached.client, &pat)).await,
            None => Some(first_contact(&reached.client, &pat).await),
        };
        let contact = match contacted {
            None => return SignInOutcome::Cancelled,
            Some(Err(failure)) => return SignInOutcome::Failed(failure),
            Some(Ok(contact)) => contact,
        };
        let credential = match SealedString::seal(&*self.inner.sealer, pat.expose()) {
            Ok(credential) => credential,
            Err(error) => {
                tracing::warn!(error = %error, "the personal access token could not be sealed");
                return SignInOutcome::Failed(SignInFailure::Insecure);
            },
        };
        drop(pat);
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return SignInOutcome::Cancelled;
        }
        match self.commit_sign_in(reached, contact, credential, keep) {
            Ok(account) => {
                tracing::info!(account = %account, keep, "signed in");
                SignInOutcome::SignedIn
            },
            Err(error) => {
                tracing::warn!(error = %error, "the signed-in account could not be stored");
                SignInOutcome::Failed(SignInFailure::Failed)
            },
        }
    }

    fn commit_sign_in(
        &self,
        reached: &Reached,
        contact: FirstContact,
        credential: SealedString,
        keep: bool,
    ) -> Result<AccountId, AccountsError> {
        let FirstContact {
            session,
            whoami,
            key,
            summary,
        } = contact;
        let pat = keep.then(|| credential.clone());
        let arrival = Arrival::new(reached, whoami.as_ref(), key, pat);
        let account = self.mutate_store(|store, _| arrival.upsert(store))?;
        self.remember_sign_in(&account, keep, &credential, summary);
        self.drop_session(&account);
        if let Err(error) = self.install_session(&account, session, credential) {
            tracing::debug!(
                account = %account,
                error = %error,
                "the first session token was not kept"
            );
        }
        self.publish();
        Ok(account)
    }

    fn remember_sign_in(
        &self,
        account: &AccountId,
        keep: bool,
        credential: &SealedString,
        summary: Option<InstanceSummary>,
    ) {
        let mut state = self.inner.state.lock();
        if keep {
            state.volatile.remove(account);
        } else {
            state.volatile.insert(account.clone(), credential.clone());
        }
        match summary {
            Some(summary) => state.summaries.insert(account.clone(), summary),
            None => state.summaries.remove(account),
        };
        state.last_signed_in = Some(account.clone());
    }
}

impl Arrival {
    fn new(
        reached: &Reached,
        whoami: Option<&WhoamiResponse>,
        key: Option<ManifestKey>,
        pat: Option<SealedString>,
    ) -> Self {
        Self {
            base: reached.client.base().to_owned(),
            label: label_for(reached.discovery.display_name.as_deref(), &reached.client),
            user: whoami.and_then(|who| who.email.clone().or_else(|| who.display_name.clone())),
            user_id: whoami.and_then(|who| who.user_id.clone()),
            org: whoami.and_then(|who| who.tenant_id.as_ref().map(ToString::to_string)),
            pat,
            key,
        }
    }

    fn upsert(&self, store: &mut StoreFile) -> Result<AccountId, AccountsError> {
        if !store.accounts.iter().any(|record| record.url == self.base) {
            let instance_key = InstanceKey::new(unique_slug(&self.label, &store.taken_keys()));
            let id = AccountId::new(Uuid::new_v4().to_string());
            let label = self.label.clone();
            let record = AccountRecord::new(id, label, self.base.clone(), instance_key, Utc::now());
            store.accounts.push(record);
        }
        let no_active = store.active().is_none();
        let Some(record) = store
            .accounts
            .iter_mut()
            .find(|record| record.url == self.base)
        else {
            let account = AccountId::new(self.base.clone());
            return Err(AccountsError::UnknownAccount { account });
        };
        if self.user_id.is_some() && record.user_id.is_some() && record.user_id != self.user_id {
            record.forget_user_state();
        }
        record.label.clone_from(&self.label);
        record.user = self.user.clone().or_else(|| record.user.take());
        record.user_id = self.user_id.clone().or_else(|| record.user_id.take());
        record.org = self.org.clone().or_else(|| record.org.take());
        record.pat.clone_from(&self.pat);
        if let Some(key) = &self.key {
            keys::offer(record, key);
        }
        if no_active {
            record.active = true;
        }
        Ok(record.id.clone())
    }
}

async fn first_contact(client: &SpClient, pat: &Pat) -> Result<FirstContact, SignInFailure> {
    let session = client.mint_session(pat, None).await.map_err(|error| {
        tracing::info!(error = %error, "no session token for the new credential");
        error.sign_in_failure()
    })?;
    let (whoami, key) = tokio::join!(client.whoami(&session.jwt), client.pubkey());
    let whoami = match whoami {
        Ok(whoami) => Some(whoami),
        Err(error) if error.refuses_token() => return Err(SignInFailure::Refused),
        Err(error) => {
            tracing::info!(error = %error, "the instance did not say who signed in");
            None
        },
    };
    let key = match key {
        Ok(key) => Some(key),
        Err(error) => {
            tracing::info!(error = %error, "the instance served no manifest key");
            None
        },
    };
    let summary = match &key {
        Some(key) => first_summary(client, &session, key).await,
        None => None,
    };
    Ok(FirstContact {
        session,
        whoami,
        key,
        summary,
    })
}

async fn first_summary(
    client: &SpClient,
    session: &JwtSession,
    key: &ManifestKey,
) -> Option<InstanceSummary> {
    match client.manifest(&session.jwt, key, None, true).await {
        Ok(ManifestFetch::Fetched { manifest, .. }) => Some(summarize(&manifest)),
        Ok(ManifestFetch::NotModified) => None,
        Err(error) => {
            tracing::info!(error = %error, "the first manifest was not summarised");
            None
        },
    }
}
