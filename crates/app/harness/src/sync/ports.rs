//! The calls the sync engine makes on an instance and on the accounts, as
//! traits so the engine runs against fakes: `Instance` is implemented for
//! `SpClient`, `Store` for `Accounts`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::sync::Arc;

use systemprompt_device::accounts::{Accounts, AccountsError, DeviceEntry};
use systemprompt_device::client::{
    ClientError, ConsumerOutcome, CredentialError, CredentialSource, Freshness, JwtSession,
    ManifestFetch, ManifestKey, SpClient,
};
use systemprompt_device::protocol::ids::{AccountId, InstallId, InstanceKey};
use systemprompt_device::protocol::wire::DeviceEnrolment;
use systemprompt_device::protocol::{DeviceCredential, Jwt};
use systemprompt_identifiers::{PluginId, UserId};
use systemprompt_models::bridge::catalog::ModelCatalog;
use systemprompt_models::bridge::consumer::SessionBinding;
use systemprompt_models::bridge::gateway::SelfEnrollRequest as DeviceEnrolRequest;
use systemprompt_models::bridge::heartbeat::{HeartbeatRequest, HeartbeatResponse};
use systemprompt_models::bridge::manifest::ManagedPublicationRef;
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_models::bridge::profile::BridgeProfileResponse as Profile;
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptRequest, ConsumerReceiptResponse,
    SessionBindingRequest,
};

/// The calls the engine makes on one systemprompt instance; implemented for
/// `SpClient`.
pub trait Instance: Clone + Send + Sync + 'static {
    fn base(&self) -> &str;

    fn pubkey(&self) -> impl Future<Output = Result<ManifestKey, ClientError>> + Send;

    fn manifest(
        &self,
        jwt: &Jwt,
        key: &ManifestKey,
        etag: Option<&str>,
        no_cache: bool,
    ) -> impl Future<Output = Result<ManifestFetch, ClientError>> + Send;

    fn profile(&self, jwt: &Jwt) -> impl Future<Output = Result<Profile, ClientError>> + Send;

    fn models(&self, jwt: &Jwt) -> impl Future<Output = Result<ModelCatalog, ClientError>> + Send;

    fn plugin_file(
        &self,
        jwt: &Jwt,
        plugin: &PluginId,
        path: &str,
    ) -> impl Future<Output = Result<Vec<u8>, ClientError>> + Send;

    fn enrol_device(
        &self,
        jwt: &Jwt,
        request: &DeviceEnrolRequest,
    ) -> impl Future<Output = Result<DeviceEnrolment, ClientError>> + Send;

    fn installation_plan(
        &self,
        device: &DeviceCredential,
        publication: &ManagedPublicationRef,
    ) -> impl Future<Output = ConsumerOutcome<ConsumerInstallationPlan>> + Send;

    fn post_receipt(
        &self,
        device: &DeviceCredential,
        receipt: &ConsumerReceiptRequest,
    ) -> impl Future<Output = ConsumerOutcome<ConsumerReceiptResponse>> + Send;

    fn post_session_binding(
        &self,
        device: &DeviceCredential,
        binding: &SessionBindingRequest,
    ) -> impl Future<Output = ConsumerOutcome<SessionBinding>> + Send;

    fn heartbeat(
        &self,
        jwt: &Jwt,
        request: &HeartbeatRequest,
    ) -> impl Future<Output = Result<HeartbeatResponse, ClientError>> + Send;
}

impl Instance for SpClient {
    fn base(&self) -> &str {
        Self::base(self)
    }

    fn pubkey(&self) -> impl Future<Output = Result<ManifestKey, ClientError>> + Send {
        Self::pubkey(self)
    }

    fn manifest(
        &self,
        jwt: &Jwt,
        key: &ManifestKey,
        etag: Option<&str>,
        no_cache: bool,
    ) -> impl Future<Output = Result<ManifestFetch, ClientError>> + Send {
        Self::manifest(self, jwt, key, etag, no_cache)
    }

    fn profile(&self, jwt: &Jwt) -> impl Future<Output = Result<Profile, ClientError>> + Send {
        Self::profile(self, jwt)
    }

    fn models(&self, jwt: &Jwt) -> impl Future<Output = Result<ModelCatalog, ClientError>> + Send {
        Self::models(self, jwt)
    }

    fn plugin_file(
        &self,
        jwt: &Jwt,
        plugin: &PluginId,
        path: &str,
    ) -> impl Future<Output = Result<Vec<u8>, ClientError>> + Send {
        Self::plugin_file(self, jwt, plugin, path)
    }

    fn enrol_device(
        &self,
        jwt: &Jwt,
        request: &DeviceEnrolRequest,
    ) -> impl Future<Output = Result<DeviceEnrolment, ClientError>> + Send {
        Self::enrol_device(self, jwt, request)
    }

    fn installation_plan(
        &self,
        device: &DeviceCredential,
        publication: &ManagedPublicationRef,
    ) -> impl Future<Output = ConsumerOutcome<ConsumerInstallationPlan>> + Send {
        Self::installation_plan(self, device, publication)
    }

    fn post_receipt(
        &self,
        device: &DeviceCredential,
        receipt: &ConsumerReceiptRequest,
    ) -> impl Future<Output = ConsumerOutcome<ConsumerReceiptResponse>> + Send {
        Self::post_receipt(self, device, receipt)
    }

    fn post_session_binding(
        &self,
        device: &DeviceCredential,
        binding: &SessionBindingRequest,
    ) -> impl Future<Output = ConsumerOutcome<SessionBinding>> + Send {
        Self::post_session_binding(self, device, binding)
    }

    fn heartbeat(
        &self,
        jwt: &Jwt,
        request: &HeartbeatRequest,
    ) -> impl Future<Output = Result<HeartbeatResponse, ClientError>> + Send {
        Self::heartbeat(self, jwt, request)
    }
}

/// The account state the engine reads and records; implemented for `Accounts`.
pub trait Store: Clone + Send + Sync + 'static {
    type Instance: Instance;

    fn instance(&self, account: &AccountId) -> Result<Self::Instance, AccountsError>;

    fn credentials(&self, account: &AccountId) -> Arc<dyn CredentialSource>;

    fn session(
        &self,
        account: &AccountId,
        freshness: Freshness,
    ) -> impl Future<Output = Result<JwtSession, CredentialError>> + Send;

    fn pinned_key(&self, account: &AccountId) -> Result<Option<ManifestKey>, AccountsError>;

    fn offer_key(&self, account: &AccountId, key: &ManifestKey) -> Result<(), AccountsError>;

    fn last_applied_version(
        &self,
        account: &AccountId,
    ) -> Result<Option<ManifestVersion>, AccountsError>;

    fn record_applied_version(
        &self,
        account: &AccountId,
        version: &ManifestVersion,
    ) -> Result<(), AccountsError>;

    fn user_id(&self, account: &AccountId) -> Result<Option<UserId>, AccountsError>;

    fn instance_key(&self, account: &AccountId) -> Result<InstanceKey, AccountsError>;

    fn install_id(&self) -> InstallId;

    fn device(&self, account: &AccountId) -> Result<Option<DeviceEntry>, AccountsError>;

    fn set_device(&self, account: &AccountId, device: &DeviceEntry) -> Result<(), AccountsError>;

    fn forget_device(&self, account: &AccountId) -> Result<(), AccountsError>;

    fn outbox(&self, account: &AccountId) -> Result<Option<String>, AccountsError>;

    fn set_outbox(&self, account: &AccountId, pointer: Option<String>)
    -> Result<(), AccountsError>;
}

impl Store for Accounts {
    type Instance = SpClient;

    fn instance(&self, account: &AccountId) -> Result<SpClient, AccountsError> {
        self.client(account)
    }

    fn credentials(&self, account: &AccountId) -> Arc<dyn CredentialSource> {
        Self::credentials(self, account)
    }

    fn session(
        &self,
        account: &AccountId,
        freshness: Freshness,
    ) -> impl Future<Output = Result<JwtSession, CredentialError>> + Send {
        Self::session(self, account, freshness)
    }

    fn pinned_key(&self, account: &AccountId) -> Result<Option<ManifestKey>, AccountsError> {
        Self::pinned_key(self, account)
    }

    fn offer_key(&self, account: &AccountId, key: &ManifestKey) -> Result<(), AccountsError> {
        Self::offer_key(self, account, key)
    }

    fn last_applied_version(
        &self,
        account: &AccountId,
    ) -> Result<Option<ManifestVersion>, AccountsError> {
        Self::last_applied_version(self, account)
    }

    fn record_applied_version(
        &self,
        account: &AccountId,
        version: &ManifestVersion,
    ) -> Result<(), AccountsError> {
        Self::record_applied_version(self, account, version)
    }

    fn user_id(&self, account: &AccountId) -> Result<Option<UserId>, AccountsError> {
        Self::user_id(self, account)
    }

    fn instance_key(&self, account: &AccountId) -> Result<InstanceKey, AccountsError> {
        Self::instance_key(self, account)
    }

    fn install_id(&self) -> InstallId {
        Self::install_id(self)
    }

    fn device(&self, account: &AccountId) -> Result<Option<DeviceEntry>, AccountsError> {
        Self::device(self, account)
    }

    fn set_device(&self, account: &AccountId, device: &DeviceEntry) -> Result<(), AccountsError> {
        Self::set_device(self, account, device)
    }

    fn forget_device(&self, account: &AccountId) -> Result<(), AccountsError> {
        Self::forget_device(self, account)
    }

    fn outbox(&self, account: &AccountId) -> Result<Option<String>, AccountsError> {
        Self::outbox(self, account)
    }

    fn set_outbox(
        &self,
        account: &AccountId,
        pointer: Option<String>,
    ) -> Result<(), AccountsError> {
        Self::set_outbox(self, account, pointer)
    }
}
