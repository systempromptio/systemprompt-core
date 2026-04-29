use super::{CredentialsOutcome, InstallOptions, ManagedProfileOutcome, UninstallSummary};
use crate::ids::PinnedPubKey;
use crate::schedule::Os;
use std::path::PathBuf;
use systemprompt_identifiers::ValidatedUrl;

#[derive(Default)]
pub struct InstallOptionsBuilder {
    print_mdm: Option<Os>,
    emit_schedule_template: Option<Os>,
    gateway_url: Option<ValidatedUrl>,
    pubkey: Option<PinnedPubKey>,
    apply: bool,
    apply_mobileconfig: bool,
}

impl InstallOptionsBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn print_mdm(mut self, os: Os) -> Self {
        self.print_mdm = Some(os);
        self
    }

    #[must_use]
    pub fn emit_schedule_template(mut self, os: Os) -> Self {
        self.emit_schedule_template = Some(os);
        self
    }

    #[must_use]
    pub fn gateway_url(mut self, url: ValidatedUrl) -> Self {
        self.gateway_url = Some(url);
        self
    }

    #[must_use]
    pub fn pubkey(mut self, pubkey: PinnedPubKey) -> Self {
        self.pubkey = Some(pubkey);
        self
    }

    #[must_use]
    pub const fn apply(mut self, apply: bool) -> Self {
        self.apply = apply;
        self
    }

    #[must_use]
    pub const fn apply_mobileconfig(mut self, apply_mobileconfig: bool) -> Self {
        self.apply_mobileconfig = apply_mobileconfig;
        self
    }

    #[must_use]
    pub fn build(self) -> InstallOptions {
        InstallOptions {
            print_mdm: self.print_mdm,
            emit_schedule_template: self.emit_schedule_template,
            gateway_url: self.gateway_url,
            pubkey: self.pubkey,
            apply: self.apply,
            apply_mobileconfig: self.apply_mobileconfig,
        }
    }
}

pub struct UninstallSummaryBuilder {
    metadata_removed: Option<PathBuf>,
    metadata_already_clean: Option<PathBuf>,
    managed_profile: ManagedProfileOutcome,
    credentials: CredentialsOutcome,
}

impl UninstallSummaryBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            metadata_removed: None,
            metadata_already_clean: None,
            managed_profile: ManagedProfileOutcome::NotApplicable,
            credentials: CredentialsOutcome::Kept,
        }
    }

    #[must_use]
    pub fn metadata_removed(mut self, path: PathBuf) -> Self {
        self.metadata_removed = Some(path);
        self
    }

    #[must_use]
    pub fn metadata_already_clean(mut self, path: PathBuf) -> Self {
        self.metadata_already_clean = Some(path);
        self
    }

    #[must_use]
    pub fn managed_profile(mut self, outcome: ManagedProfileOutcome) -> Self {
        self.managed_profile = outcome;
        self
    }

    #[must_use]
    pub fn credentials(mut self, outcome: CredentialsOutcome) -> Self {
        self.credentials = outcome;
        self
    }

    #[must_use]
    pub fn build(self) -> UninstallSummary {
        UninstallSummary {
            metadata_removed: self.metadata_removed,
            metadata_already_clean: self.metadata_already_clean,
            managed_profile: self.managed_profile,
            credentials: self.credentials,
        }
    }
}

impl Default for UninstallSummaryBuilder {
    fn default() -> Self {
        Self::new()
    }
}
