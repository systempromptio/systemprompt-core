//! [`push_channels`]: the push channels a declaration and the secrets held
//! for it amount to.
//!
//! A provider the declaration does not name contributes nothing. A declared
//! provider whose secret is not set contributes nothing either, with a
//! warning: an instance without the credential runs without that push. A
//! declared provider whose secret is set and unusable is an error, so a
//! mistyped key refuses the build instead of silently sending no push.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_extension::notifications::{DynPushTokenDirectory, NotificationChannel};
use systemprompt_traits::SecretStore;

use super::{ForegroundPresence, ProviderHttp, PushChannel, PushError};
use crate::channels::apns::{ApnsSender, ApnsSigner};
use crate::channels::fcm::{FcmAccess, FcmSender};
use crate::plane::push::{ApnsDeclaration, FcmDeclaration, PushDeclaration};

/// What every push channel is built over.
#[derive(Clone)]
pub struct PushParts {
    pub secrets: Arc<dyn SecretStore>,
    pub directory: DynPushTokenDirectory,
    pub presence: Arc<dyn ForegroundPresence>,
    pub http: Arc<dyn ProviderHttp>,
}

impl fmt::Debug for PushParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PushParts").finish_non_exhaustive()
    }
}

pub fn push_channels(
    declaration: &PushDeclaration,
    parts: &PushParts,
) -> Result<Vec<Arc<dyn NotificationChannel>>, PushError> {
    let mut channels: Vec<Arc<dyn NotificationChannel>> = Vec::new();
    if let Some(apns) = &declaration.apns
        && let Some(sender) = configured(apns_sender(apns, parts))?
    {
        channels.push(Arc::new(PushChannel::new(
            sender,
            Arc::clone(&parts.directory),
            Arc::clone(&parts.presence),
        )));
    }
    if let Some(fcm) = &declaration.fcm
        && let Some(sender) = configured(fcm_sender(fcm, parts))?
    {
        channels.push(Arc::new(PushChannel::new(
            sender,
            Arc::clone(&parts.directory),
            Arc::clone(&parts.presence),
        )));
    }
    Ok(channels)
}

fn apns_sender(declaration: &ApnsDeclaration, parts: &PushParts) -> Result<ApnsSender, PushError> {
    let signer = ApnsSigner::new(Arc::clone(&parts.secrets), declaration);
    signer.sign()?;
    Ok(ApnsSender::new(
        Arc::clone(&parts.http),
        Arc::new(signer),
        declaration.environment,
        declaration.topic.clone(),
    ))
}

fn fcm_sender(declaration: &FcmDeclaration, parts: &PushParts) -> Result<FcmSender, PushError> {
    let access = FcmAccess::new(
        Arc::clone(&parts.secrets),
        declaration.service_account_secret.clone(),
    );
    let project = access.project()?;
    Ok(FcmSender::new(
        Arc::clone(&parts.http),
        Arc::new(access),
        project,
    ))
}

fn configured<S>(built: Result<S, PushError>) -> Result<Option<S>, PushError> {
    match built {
        Ok(sender) => Ok(Some(sender)),
        Err(PushError::CredentialMissing { secret }) => {
            tracing::warn!(
                secret = %secret,
                "Push is declared but its credential secret is not set; no push channel is contributed for it"
            );
            Ok(None)
        },
        Err(error) => Err(error),
    }
}
