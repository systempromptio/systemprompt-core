//! [`Instances`]: finding an instance from its address.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_client::SystempromptClient;
use systemprompt_models::bridge::clients::DiscoveryClients;
use systemprompt_models::bridge::instance::{BridgeDiscovery, DISCOVERY_CONTRACT};

use crate::address::InstanceAddress;
use crate::error::MobileError;
use crate::tls;

/// Where an instance offers one platform's app and the oldest version it
/// still supports.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AppRelease {
    pub app_id: String,
    pub min_version: Option<String>,
    pub location: String,
}

/// What an instance says about itself before anyone has signed in.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InstanceDiscovery {
    pub address: String,
    pub display_name: Option<String>,
    pub sign_in_hint: Option<String>,
    pub ios: Option<AppRelease>,
    pub android: Option<AppRelease>,
}

impl InstanceDiscovery {
    fn from_document(address: &InstanceAddress, document: BridgeDiscovery) -> Self {
        let DiscoveryClients { ios, android } = document.clients;
        Self {
            address: address.as_str().to_owned(),
            display_name: document.branding.display_name,
            sign_in_hint: document.branding.sign_in_hint,
            ios: ios.map(|client| AppRelease {
                app_id: client.app_id.as_str().to_owned(),
                min_version: client.min_version.map(|v| v.to_string()),
                location: client.link,
            }),
            android: android.map(|client| AppRelease {
                app_id: client.app_id.as_str().to_owned(),
                min_version: client.min_version.map(|v| v.to_string()),
                location: address.join(&client.download_path),
            }),
        }
    }
}

/// The instances this app knows: the one it last discovered is the active one.
#[derive(Debug, uniffi::Object)]
pub struct Instances {
    active: Mutex<Option<InstanceDiscovery>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl Instances {
    #[uniffi::constructor]
    #[must_use]
    pub fn new() -> Arc<Self> {
        tls::install_crypto_provider();
        Arc::new(Self {
            active: Mutex::new(None),
        })
    }

    /// Reads the discovery document of the instance at `address`.
    pub async fn discover(&self, address: String) -> Result<InstanceDiscovery, MobileError> {
        let address = InstanceAddress::parse(&address)?;
        let document = SystempromptClient::new(address.as_str())?
            .get_bridge_discovery()
            .await?;
        if document.contract != DISCOVERY_CONTRACT {
            return Err(MobileError::VersionTooOld);
        }
        let discovery = InstanceDiscovery::from_document(&address, document);
        *self.active.lock().unwrap_or_else(PoisonError::into_inner) = Some(discovery.clone());
        Ok(discovery)
    }

    /// The instance the last successful [`Instances::discover`] read.
    #[must_use]
    pub fn active(&self) -> Option<InstanceDiscovery> {
        self.active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
