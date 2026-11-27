//! A non-interactive sign-in to a named instance with a personal access token
//! read from standard input: the `login --stdin` command.
//!
//! The token is kept sealed for the next process. A sign-in only ever offers
//! the instance's manifest key; with nobody to show it to, this command pins
//! the key the instance serves on first contact and reports its fingerprint.
//! `--fingerprint` names the key expected instead: any other key is refused,
//! and it is the only way to accept a key that replaces a pinned one. A
//! sign-in whose key is refused is signed out again, so it leaves no
//! credential behind. The signed-in account becomes the active one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::io::BufRead;
use std::path::PathBuf;

use serde::Serialize;
use systemprompt_device::accounts::Accounts;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::secure::Zeroizing;
use systemprompt_device::view::{KeyFingerprint, SignInFailure, SignInOutcome};

use crate::command::args::LoginArgs;
use crate::command::compose::Composition;
use crate::command::exit::ExitStatus;
use crate::command::host::HeadlessHost;
use crate::command::stop::describe;
use crate::command::usage::UsageError;

/// One non-interactive sign-in.
pub struct LoginRequest {
    pub instance: String,
    pub token: Zeroizing<String>,
    pub fingerprint: Option<KeyFingerprint>,
    pub data_root: Option<PathBuf>,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginRequest")
            .field("instance", &self.instance)
            .field("fingerprint", &self.fingerprint)
            .field("data_root", &self.data_root)
            .finish_non_exhaustive()
    }
}

impl LoginArgs {
    pub fn request(self, token: Zeroizing<String>) -> LoginRequest {
        LoginRequest {
            instance: self.instance,
            token,
            fingerprint: self.fingerprint,
            data_root: self.data_root,
        }
    }
}

/// Why a sign-in did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginFailure {
    Unreachable,
    Refused,
    RateLimited,
    Incompatible,
    Insecure,
    Failed,
    KeyMismatch,
    KeyChanged,
    Storage,
}

impl LoginFailure {
    const fn of_sign_in(failure: SignInFailure) -> Self {
        match failure {
            SignInFailure::Unreachable => Self::Unreachable,
            SignInFailure::Refused => Self::Refused,
            SignInFailure::RateLimited => Self::RateLimited,
            SignInFailure::Incompatible => Self::Incompatible,
            SignInFailure::Insecure => Self::Insecure,
            SignInFailure::Failed => Self::Failed,
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::Unreachable => "the instance could not be reached",
            Self::Refused => "the instance refused the token",
            Self::RateLimited => "the instance is rate limiting sign-ins",
            Self::Incompatible => "the address is not a supported instance",
            Self::Insecure => "no secure storage is available to keep the token",
            Self::Failed => "the sign-in failed",
            Self::KeyMismatch => "the instance does not serve the manifest key that was named",
            Self::KeyChanged => {
                "the instance serves another manifest key than the pinned one; name it with \
                 --fingerprint to accept it"
            },
            Self::Storage => "the account store could not be read or written",
        }
    }

    const fn status(self) -> ExitStatus {
        match self {
            Self::Unreachable | Self::RateLimited => ExitStatus::Unavailable,
            Self::Refused | Self::KeyMismatch | Self::KeyChanged => ExitStatus::Refused,
            Self::Incompatible | Self::Insecure | Self::Failed => ExitStatus::Failed,
            Self::Storage => ExitStatus::Internal,
        }
    }
}

/// What a sign-in ended in. `key_fingerprint` is the pinned manifest key,
/// absent when the instance served none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum LoginOutcome {
    SignedIn {
        account: AccountId,
        instance: String,
        user: Option<String>,
        key_fingerprint: Option<KeyFingerprint>,
    },
    Failed {
        reason: LoginFailure,
        message: String,
    },
}

/// The outcome of `login` and the status the process exits with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginReport {
    pub status: ExitStatus,
    pub outcome: LoginOutcome,
}

#[derive(Serialize)]
struct LoginLine<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    exit_code: u8,
    #[serde(flatten)]
    outcome: &'a LoginOutcome,
}

impl LoginReport {
    fn failed(reason: LoginFailure) -> Self {
        Self::failed_with(reason, reason.message().to_owned())
    }

    const fn failed_with(reason: LoginFailure, message: String) -> Self {
        Self {
            status: reason.status(),
            outcome: LoginOutcome::Failed { reason, message },
        }
    }

    pub fn encode(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&LoginLine {
            kind: "login",
            exit_code: self.status.code(),
            outcome: &self.outcome,
        })
    }
}

pub fn read_token(input: &mut impl BufRead) -> Result<Zeroizing<String>, UsageError> {
    let mut line = Zeroizing::new(String::new());
    input.read_line(&mut line).map_err(UsageError::Stdin)?;
    let token = Zeroizing::new(line.trim().to_owned());
    if token.is_empty() {
        return Err(UsageError::EmptyToken);
    }
    Ok(token)
}

pub async fn login(host: &HeadlessHost, request: LoginRequest) -> LoginReport {
    let root = host.root(request.data_root.as_deref());
    let accounts = match Composition::open_accounts(host, &root) {
        Ok(accounts) => accounts,
        Err(error) => return LoginReport::failed_with(LoginFailure::Storage, describe(&error)),
    };
    let report = sign_in(&accounts, &request).await;
    accounts.shutdown();
    report
}

async fn sign_in(accounts: &Accounts, request: &LoginRequest) -> LoginReport {
    let token = request.token.as_str().to_owned();
    match accounts.sign_in_pat(&request.instance, token, true).await {
        SignInOutcome::SignedIn => {},
        SignInOutcome::Failed(failure) => {
            return LoginReport::failed(LoginFailure::of_sign_in(failure));
        },
        SignInOutcome::Cancelled | SignInOutcome::Denied | SignInOutcome::Expired => {
            return LoginReport::failed(LoginFailure::Failed);
        },
    }
    let Some(id) = accounts.snapshot().last_signed_in else {
        return LoginReport::failed(LoginFailure::Storage);
    };
    let pinned = match pin(accounts, &id, request.fingerprint.as_ref()) {
        Ok(pinned) => pinned,
        Err(reason) => {
            if let Err(error) = accounts.sign_out(&id) {
                tracing::warn!(account = %id, error = %error, "the refused sign-in was not signed out");
            }
            return LoginReport::failed(reason);
        },
    };
    if let Err(error) = accounts.activate(&id) {
        return LoginReport::failed_with(LoginFailure::Storage, describe(&error));
    }
    let view = accounts.get(&id);
    LoginReport {
        status: ExitStatus::Completed,
        outcome: LoginOutcome::SignedIn {
            instance: view
                .as_ref()
                .map_or_else(|| request.instance.clone(), |view| view.url.clone()),
            user: view.and_then(|view| view.user),
            account: id,
            key_fingerprint: pinned,
        },
    }
}

fn pin(
    accounts: &Accounts,
    id: &AccountId,
    expected: Option<&KeyFingerprint>,
) -> Result<Option<KeyFingerprint>, LoginFailure> {
    let storage = |error| {
        tracing::warn!(account = %id, error = %error, "the manifest key could not be pinned");
        LoginFailure::Storage
    };
    let Some(offer) = accounts.key_offer(id).map_err(storage)? else {
        let pinned = accounts.get(id).and_then(|view| view.pinned);
        return match expected {
            Some(expected) if pinned.as_ref() != Some(expected) => Err(LoginFailure::KeyMismatch),
            _ => Ok(pinned),
        };
    };
    match expected {
        Some(expected) if expected != &offer.offered => return Err(LoginFailure::KeyMismatch),
        None if offer.pinned.is_some() => return Err(LoginFailure::KeyChanged),
        _ => {},
    }
    accounts.trust(id, &offer.offered).map_err(storage)?;
    Ok(Some(offer.offered))
}
