//! The [`Accounts`] handle: the store, the in-memory state beside it and the
//! change feed.
//!
//! Every store change goes through one writer: the next store is computed on a
//! copy, written owner-only, and only then becomes the state, so a failed write
//! changes nothing. Memory-only state (session tokens, PATs kept for this run,
//! summaries, the browser sign-in in progress) sits beside it under a short
//! lock that is never held across an await. A missing store file is an
//! empty store; one that does not decode fails the open and is left untouched.
//! The accounts are opened inside the tokio runtime they run on, since their
//! renewal tasks are spawned there.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use crate::client::HttpClient;
use sp_protocol::ids::{AccountId, InstallId};
use sp_protocol::view::AccountView;
use crate::secure::Sealer;
use tokio::runtime::Handle;
use tokio::sync::{broadcast, watch};
use tokio_util::sync::CancellationToken;

use crate::accounts::browser::Browser;
use crate::accounts::error::AccountsError;
use crate::accounts::session::Renewed;
use crate::accounts::snapshot::{AccountsSnapshot, view};
use crate::accounts::state::State;
use crate::accounts::store::{self, AccountRecord, StoreFile};

const RENEWED_CAPACITY: usize = 32;

const UNNAMED_COMPUTER: &str = "this computer";

/// What [`Accounts::open`] is built from.
///
/// `device_name` is the name a minted PAT gets in the
/// user's token list, the build's device label for this computer (see
/// [`computer_name`]); the caller names it, since the product's identity lives
/// above this crate.
#[derive(Debug)]
pub struct AccountsConfig {
    pub http: HttpClient,
    pub sealer: Arc<dyn Sealer>,
    pub state_file: PathBuf,
    pub product: String,
    pub install_id: InstallId,
    pub browser: Arc<dyn Browser>,
    pub device_name: String,
}

/// The systemprompt accounts of this install. Cheap to clone; every clone is
/// the same accounts.
#[derive(Clone)]
pub struct Accounts {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) http: HttpClient,
    pub(crate) sealer: Arc<dyn Sealer>,
    pub(crate) path: PathBuf,
    pub(crate) product: String,
    pub(crate) device_name: String,
    pub(crate) install_id: InstallId,
    pub(crate) browser: Arc<dyn Browser>,
    pub(crate) runtime: Option<Handle>,
    pub(crate) secure_storage: bool,
    pub(crate) state: Mutex<State>,
    pub(crate) writer: Mutex<()>,
    pub(crate) mint_locks: Mutex<HashMap<AccountId, Arc<tokio::sync::Mutex<()>>>>,
    pub(crate) snapshot: watch::Sender<AccountsSnapshot>,
    pub(crate) renewed: broadcast::Sender<Renewed>,
    pub(crate) shutdown: CancellationToken,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

impl fmt::Debug for Accounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.inner.state.lock();
        f.debug_struct("Accounts")
            .field("path", &self.inner.path)
            .field("accounts", &state.store.accounts.len())
            .field("sessions", &state.sessions.len())
            .finish_non_exhaustive()
    }
}

#[must_use]
pub fn computer_name() -> String {
    hostname::get()
        .ok()
        .and_then(|name| name.into_string().ok())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| UNNAMED_COMPUTER.to_owned())
}

impl Accounts {
    pub fn open(config: AccountsConfig) -> Result<Self, AccountsError> {
        let (file, install_id) = prepare_store(&config.state_file, config.install_id)?;
        let secure_storage = config.sealer.available();
        if !secure_storage {
            tracing::warn!("the OS secure store is unavailable; no sign-in will start");
        }
        let (snapshot, _) = watch::channel(AccountsSnapshot::default());
        let (renewed, _) = broadcast::channel(RENEWED_CAPACITY);
        let count = file.accounts.len();
        let inner = Inner {
            http: config.http,
            sealer: config.sealer,
            path: config.state_file,
            product: config.product,
            device_name: config.device_name,
            install_id,
            browser: config.browser,
            runtime: Handle::try_current().ok(),
            secure_storage,
            state: Mutex::new(State {
                store: file,
                ..State::default()
            }),
            writer: Mutex::new(()),
            mint_locks: Mutex::new(HashMap::new()),
            snapshot,
            renewed,
            shutdown: CancellationToken::new(),
        };
        let accounts = Self {
            inner: Arc::new(inner),
        };
        accounts.publish();
        tracing::info!(accounts = count, "accounts opened");
        Ok(accounts)
    }

    #[must_use]
    pub fn install_id(&self) -> InstallId {
        self.inner.install_id.clone()
    }

    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.inner.device_name
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<AccountsSnapshot> {
        self.inner.snapshot.subscribe()
    }

    #[must_use]
    pub fn snapshot(&self) -> AccountsSnapshot {
        self.inner.snapshot.borrow().clone()
    }

    #[must_use]
    pub fn list(&self) -> Vec<AccountView> {
        let state = self.inner.state.lock();
        state
            .store
            .accounts
            .iter()
            .map(|record| view(record, state.is_signed_in(record)))
            .collect()
    }

    #[must_use]
    pub fn get(&self, id: &AccountId) -> Option<AccountView> {
        let state = self.inner.state.lock();
        state
            .store
            .find(id)
            .map(|record| view(record, state.is_signed_in(record)))
    }

    #[must_use]
    pub fn active(&self) -> Option<AccountView> {
        let state = self.inner.state.lock();
        state
            .store
            .active()
            .map(|record| view(record, state.is_signed_in(record)))
    }

    #[must_use]
    pub fn active_id(&self) -> Option<AccountId> {
        self.inner
            .state
            .lock()
            .store
            .active()
            .map(|record| record.id.clone())
    }

    pub(crate) fn read<T>(
        &self,
        id: &AccountId,
        read: impl FnOnce(&AccountRecord, &State) -> T,
    ) -> Result<T, AccountsError> {
        let state = self.inner.state.lock();
        let value = state.store.find(id).map(|record| read(record, &state));
        drop(state);
        value.ok_or_else(|| AccountsError::UnknownAccount {
            account: id.clone(),
        })
    }

    pub(crate) fn mutate_store<T>(
        &self,
        change: impl FnOnce(&mut StoreFile, &HashSet<AccountId>) -> Result<T, AccountsError>,
    ) -> Result<T, AccountsError> {
        let _writing = self.inner.writer.lock();
        let (mut next, volatile) = {
            let state = self.inner.state.lock();
            (
                state.store.clone(),
                state.volatile.keys().cloned().collect::<HashSet<_>>(),
            )
        };
        let value = change(&mut next, &volatile)?;
        let bytes = store::encode(&next)?;
        store::save(&self.inner.path, &bytes)?;
        self.inner.state.lock().store = next;
        Ok(value)
    }

    pub(crate) fn publish(&self) {
        let snapshot = self.inner.state.lock().snapshot(self.inner.secure_storage);
        self.inner.snapshot.send_replace(snapshot);
    }
}

fn prepare_store(
    path: &Path,
    install_id: InstallId,
) -> Result<(StoreFile, InstallId), AccountsError> {
    let mut file = store::load(path)?.unwrap_or_default();
    let normalized = store::normalize(&mut file);
    let minted = file.install_id.is_none();
    if minted {
        file.install_id = Some(install_id.clone());
    }
    if file.install_id.as_ref() != Some(&install_id) {
        tracing::warn!("the account store names another install id; keeping the stored one");
    }
    let install_id = file.install_id.clone().unwrap_or(install_id);
    if normalized || minted {
        store::save(path, &store::encode(&file)?)?;
    }
    Ok((file, install_id))
}
