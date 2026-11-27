//! Reads with the user's JWT: profile, model catalog, identity and plugin
//! bundle files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use crate::protocol::names::{is_plain_relative_path, is_safe_plugin_segment};
use crate::protocol::{Jwt, paths};
use systemprompt_identifiers::PluginId;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::catalog::ModelCatalog;
use systemprompt_models::bridge::gateway::WhoamiResponse;
use systemprompt_models::bridge::profile::BridgeProfileResponse as Profile;

use crate::client::client::{SpClient, push_segments};
use crate::client::error::ClientError;
use crate::client::http::{DEFAULT_TIMEOUT, SMALL_BODY, decode, exchange, failure};

const PROFILE_ENDPOINT: &str = "GET /v1/bridge/profile";
const MODELS_ENDPOINT: &str = "GET /v1/models";
const WHOAMI_ENDPOINT: &str = "GET /v1/bridge/whoami";
const PLUGIN_FILE_ENDPOINT: &str = "GET /v1/bridge/plugins/{plugin}/{path}";

const CATALOG_BODY: usize = 4 * 1024 * 1024;
const PLUGIN_FILE_BODY: usize = 8 * 1024 * 1024;
const PLUGIN_FILE_TIMEOUT: Duration = Duration::from_secs(60);

impl SpClient {
    pub async fn profile(&self, jwt: &Jwt) -> Result<Profile, ClientError> {
        let request = self
            .get(self.at(ApiPaths::BRIDGE_PROFILE)?)
            .bearer_auth(jwt.expose());
        let answer = exchange(PROFILE_ENDPOINT, request, DEFAULT_TIMEOUT, CATALOG_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(PROFILE_ENDPOINT, &answer));
        }
        decode(PROFILE_ENDPOINT, &answer)
    }

    pub async fn models(&self, jwt: &Jwt) -> Result<ModelCatalog, ClientError> {
        let request = self.get(self.at(paths::MODELS)?).bearer_auth(jwt.expose());
        let answer = exchange(MODELS_ENDPOINT, request, DEFAULT_TIMEOUT, CATALOG_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(MODELS_ENDPOINT, &answer));
        }
        decode(MODELS_ENDPOINT, &answer)
    }

    pub async fn whoami(&self, jwt: &Jwt) -> Result<WhoamiResponse, ClientError> {
        let request = self
            .get(self.at(ApiPaths::BRIDGE_WHOAMI)?)
            .bearer_auth(jwt.expose());
        let answer = exchange(WHOAMI_ENDPOINT, request, DEFAULT_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(WHOAMI_ENDPOINT, &answer));
        }
        decode(WHOAMI_ENDPOINT, &answer)
    }

    pub async fn plugin_file(
        &self,
        jwt: &Jwt,
        plugin: &PluginId,
        path: &str,
    ) -> Result<Vec<u8>, ClientError> {
        if !is_safe_plugin_segment(plugin.as_str()) {
            return Err(ClientError::InvalidInput {
                what: "the plugin id is not a single safe path segment",
            });
        }
        if !is_plain_relative_path(path) {
            return Err(ClientError::InvalidInput {
                what: "the bundle file path is not a plain relative path",
            });
        }
        let mut url = self.at(ApiPaths::BRIDGE_PLUGINS)?;
        push_segments(
            &mut url,
            std::iter::once(plugin.as_str()).chain(path.split('/')),
        )?;
        let request = self.get_bytes(url).bearer_auth(jwt.expose());
        let answer = exchange(
            PLUGIN_FILE_ENDPOINT,
            request,
            PLUGIN_FILE_TIMEOUT,
            PLUGIN_FILE_BODY,
        )
        .await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(PLUGIN_FILE_ENDPOINT, &answer));
        }
        Ok(answer.body)
    }
}
