use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sp_accounts::{AccountsError, DeviceEntry};
use sp_client::{
    ClientError, ConsumerOutcome, CredentialError, CredentialSource, Freshness, JwtSession,
    ManifestFetch, ManifestKey,
};
use sp_protocol::ids::{
    AccountId, DeviceId, InstallId, InstallationReceiptId, InstanceKey, PluginId, UserId,
};
use sp_protocol::wire::catalog::{ModelCatalog, Profile};
use sp_protocol::wire::consumer::{
    Acknowledgement, DeviceEnrolRequest, DeviceEnrolment, InstallationPlan, ReceiptRequest,
    ReceiptResponse, SessionBinding as BindingAck, SessionBindingRequest,
};
use sp_protocol::wire::heartbeat::{HeartbeatRequest, HeartbeatResponse};
use sp_protocol::wire::manifest::Publication;
use sp_protocol::{DeviceCredential, Jwt, ManifestVersion};

use sp_sync::ports::{Instance, Store};

use crate::fakes::{BASE, FakeCredentials, FakeInstance, at, plan_for, status_error, unreachable};

fn outcome<T>(status: Option<u16>, value: impl FnOnce() -> T) -> ConsumerOutcome<T> {
    ConsumerOutcome::classify(
        status.map_or_else(|| Ok(value()), |status| Err(status_error(status))),
    )
}

impl Instance for FakeInstance {
    fn base(&self) -> &str {
        BASE
    }

    fn pubkey(&self) -> impl Future<Output = Result<ManifestKey, ClientError>> + Send {
        std::future::ready(self.state().served_key.clone().ok_or_else(unreachable))
    }

    fn manifest(
        &self,
        _jwt: &Jwt,
        _key: &ManifestKey,
        etag: Option<&str>,
        no_cache: bool,
    ) -> impl Future<Output = Result<ManifestFetch, ClientError>> + Send {
        let answer = {
            let mut state = self.state();
            state
                .manifest_calls
                .push((etag.map(str::to_owned), no_cache));
            state
                .manifests
                .pop_front()
                .unwrap_or(Ok(ManifestFetch::NotModified))
        };
        std::future::ready(answer)
    }

    fn profile(&self, _jwt: &Jwt) -> impl Future<Output = Result<Profile, ClientError>> + Send {
        let answer = {
            self.state().catalog_reads += 1;
            Err(status_error(404))
        };
        std::future::ready(answer)
    }

    fn models(&self, _jwt: &Jwt) -> impl Future<Output = Result<ModelCatalog, ClientError>> + Send {
        std::future::ready(Err(status_error(404)))
    }

    fn plugin_file(
        &self,
        _jwt: &Jwt,
        plugin: &PluginId,
        path: &str,
    ) -> impl Future<Output = Result<Vec<u8>, ClientError>> + Send {
        let answer = {
            let mut state = self.state();
            state.file_calls += 1;
            state
                .files
                .get(&format!("{}/{path}", plugin.as_str()))
                .cloned()
                .ok_or_else(|| status_error(404))
        };
        std::future::ready(answer)
    }

    fn enrol_device(
        &self,
        _jwt: &Jwt,
        _request: &DeviceEnrolRequest,
    ) -> impl Future<Output = Result<DeviceEnrolment, ClientError>> + Send {
        let (fails, enrolments) = {
            let mut state = self.state();
            state.enrolments += 1;
            (state.enrol_fails, state.enrolments)
        };
        let answer = if fails {
            Err(status_error(403))
        } else {
            Ok(DeviceEnrolment {
                device_id: DeviceId::try_new("device-1").unwrap(),
                consumer_id: UserId::new("u-1"),
                credential: DeviceCredential::new(format!("sp_device_{enrolments}")).unwrap(),
            })
        };
        std::future::ready(answer)
    }

    fn installation_plan(
        &self,
        _device: &DeviceCredential,
        publication: &Publication,
    ) -> impl Future<Output = ConsumerOutcome<InstallationPlan>> + Send {
        let answer = {
            let mut state = self.state();
            state.plan_calls += 1;
            outcome(state.plan_status, || plan_for(publication))
        };
        std::future::ready(answer)
    }

    fn post_receipt(
        &self,
        _device: &DeviceCredential,
        receipt: &ReceiptRequest,
    ) -> impl Future<Output = ConsumerOutcome<ReceiptResponse>> + Send {
        let answer = {
            let mut state = self.state();
            state.receipts.push(receipt.clone());
            outcome(state.receipt_status, || ReceiptResponse {
                receipt_id: InstallationReceiptId::new("receipt-1"),
                acknowledgement: Acknowledgement::Accepted,
                acknowledged_at: at(0),
                fully_verified: receipt.fully_verified(),
            })
        };
        std::future::ready(answer)
    }

    fn post_session_binding(
        &self,
        _device: &DeviceCredential,
        binding: &SessionBindingRequest,
    ) -> impl Future<Output = ConsumerOutcome<BindingAck>> + Send {
        let answer = {
            let mut state = self.state();
            state.bindings.push(binding.clone());
            outcome(state.binding_status, || BindingAck {
                id: sp_protocol::ids::InstallationSessionBindingId::new("binding-1"),
                bound_at: at(0),
            })
        };
        std::future::ready(answer)
    }

    fn heartbeat(
        &self,
        _jwt: &Jwt,
        request: &HeartbeatRequest,
    ) -> impl Future<Output = Result<HeartbeatResponse, ClientError>> + Send {
        let answer = {
            let mut state = self.state();
            state.heartbeats.push(request.clone());
            state.heartbeat_answers.pop_front().unwrap_or_else(|| {
                Ok(HeartbeatResponse {
                    min_bridge_version: "0.1.0".to_owned(),
                    compatible: true,
                    window_status: None,
                })
            })
        };
        std::future::ready(answer)
    }
}

#[derive(Default)]
pub(crate) struct StoreState {
    pub(crate) pinned: Option<ManifestKey>,
    pub(crate) offered: Option<ManifestKey>,
    pub(crate) applied: Option<ManifestVersion>,
    pub(crate) user: Option<UserId>,
    pub(crate) device: Option<DeviceEntry>,
    pub(crate) outbox: Option<String>,
    pub(crate) forgotten: usize,
    pub(crate) sessions: Vec<Freshness>,
}

#[derive(Clone, Default)]
pub(crate) struct FakeStore {
    pub(crate) state: Arc<Mutex<StoreState>>,
    pub(crate) instance: FakeInstance,
}

impl FakeStore {
    pub(crate) fn state(&self) -> std::sync::MutexGuard<'_, StoreState> {
        self.state.lock().unwrap()
    }
}

impl Store for FakeStore {
    type Instance = FakeInstance;

    fn instance(&self, _account: &AccountId) -> Result<FakeInstance, AccountsError> {
        Ok(self.instance.clone())
    }

    fn credentials(&self, _account: &AccountId) -> Arc<dyn CredentialSource> {
        Arc::new(FakeCredentials)
    }

    fn session(
        &self,
        _account: &AccountId,
        freshness: Freshness,
    ) -> impl Future<Output = Result<JwtSession, CredentialError>> + Send {
        let minted = {
            let mut state = self.state();
            state.sessions.push(freshness);
            state.sessions.len()
        };
        std::future::ready(Ok(JwtSession {
            jwt: Jwt::new("header.payload.signature").unwrap(),
            ttl: Duration::from_secs(3600),
            expires_at: at(3600),
            session_id: Some(sp_protocol::ids::SessionId::new(format!("sess-{minted}"))),
            device_token: None,
        }))
    }

    fn pinned_key(&self, _account: &AccountId) -> Result<Option<ManifestKey>, AccountsError> {
        Ok(self.state().pinned.clone())
    }

    fn offer_key(&self, _account: &AccountId, key: &ManifestKey) -> Result<(), AccountsError> {
        self.state().offered = Some(key.clone());
        Ok(())
    }

    fn last_applied_version(
        &self,
        _account: &AccountId,
    ) -> Result<Option<ManifestVersion>, AccountsError> {
        Ok(self.state().applied.clone())
    }

    fn record_applied_version(
        &self,
        _account: &AccountId,
        version: &ManifestVersion,
    ) -> Result<(), AccountsError> {
        self.state().applied = Some(version.clone());
        Ok(())
    }

    fn user_id(&self, _account: &AccountId) -> Result<Option<UserId>, AccountsError> {
        Ok(self.state().user.clone())
    }

    fn instance_key(&self, _account: &AccountId) -> Result<InstanceKey, AccountsError> {
        Ok(InstanceKey::new("alpha"))
    }

    fn install_id(&self) -> InstallId {
        InstallId::new("install-1")
    }

    fn device(&self, _account: &AccountId) -> Result<Option<DeviceEntry>, AccountsError> {
        Ok(self.state().device.clone())
    }

    fn set_device(&self, _account: &AccountId, device: &DeviceEntry) -> Result<(), AccountsError> {
        self.state().device = Some(device.clone());
        Ok(())
    }

    fn forget_device(&self, _account: &AccountId) -> Result<(), AccountsError> {
        let mut state = self.state();
        state.device = None;
        state.forgotten += 1;
        drop(state);
        Ok(())
    }

    fn outbox(&self, _account: &AccountId) -> Result<Option<String>, AccountsError> {
        Ok(self.state().outbox.clone())
    }

    fn set_outbox(
        &self,
        _account: &AccountId,
        pointer: Option<String>,
    ) -> Result<(), AccountsError> {
        self.state().outbox = pointer;
        Ok(())
    }
}
