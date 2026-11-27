//! What an account's instance holds, as counted from its first manifest. The
//! manifest is verified against the key the account was offered, else the one
//! it trusts; the summary is kept in memory for the run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sp_client::ManifestFetch;
use sp_protocol::ids::AccountId;
use sp_protocol::view::InstanceSummary;
use sp_protocol::wire::manifest::Manifest;

use crate::accounts::Accounts;
use crate::error::AccountsError;
use crate::keys;

impl Accounts {
    #[must_use]
    pub fn last_summary(&self, id: &AccountId) -> Option<InstanceSummary> {
        self.inner.state.lock().summaries.get(id).cloned()
    }

    pub async fn summary(&self, id: &AccountId) -> Result<InstanceSummary, AccountsError> {
        if let Some(known) = self.last_summary(id) {
            return Ok(known);
        }
        let key = self
            .read(id, |record, _| keys::verification_key(record))?
            .ok_or_else(|| AccountsError::NoManifestKey {
                account: id.clone(),
            })?;
        let client = self.client(id)?;
        let source = self.credentials(id);
        let fetched = sp_client::with_jwt(&*source, |jwt| {
            let client = client.clone();
            let key = key.clone();
            async move { client.manifest(&jwt, &key, None, true).await }
        })
        .await?;
        let ManifestFetch::Fetched { manifest, .. } = fetched else {
            return Err(AccountsError::NoManifest {
                account: id.clone(),
            });
        };
        let summary = summarize(&manifest);
        self.inner
            .state
            .lock()
            .summaries
            .insert(id.clone(), summary.clone());
        Ok(summary)
    }
}

pub(crate) fn summarize(manifest: &Manifest) -> InstanceSummary {
    let count = |len: usize| u32::try_from(len).unwrap_or(u32::MAX);
    InstanceSummary {
        skills: count(manifest.skills.len()),
        agents: count(manifest.agents.len()),
        mcp: count(manifest.managed_mcp_servers.len()),
        role: manifest
            .user
            .as_ref()
            .and_then(|user| user.roles.first().cloned()),
    }
}
