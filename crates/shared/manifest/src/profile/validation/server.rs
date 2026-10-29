//! `server` replica-identity, capacity and shutdown-window checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{NodeRole, Profile, StorageBackend};

impl Profile {
    pub(crate) fn validate_instance_identity(&self, errors: &mut Vec<String>, is_cloud: bool) {
        if is_cloud && self.server.instance_id.is_some() {
            errors.push(
                "cloud profiles derive the replica identity from HOSTNAME; remove \
                 server.instance_id (a static id shared by replicas makes each replica evict the \
                 others' MCP registrations)"
                    .to_owned(),
            );
        }
    }

    pub(crate) fn validate_role(&self, errors: &mut Vec<String>) {
        let role = self.server.role;
        if matches!(role, NodeRole::Api | NodeRole::Worker)
            && self.storage.backend == StorageBackend::Local
            && !self.storage.shared
        {
            errors.push(format!(
                "server.role '{role}' runs beside other replicas; storage.backend 'local' needs \
                 storage.shared: true on a mount every replica sees, or use backend 'gcs' or 's3'"
            ));
        }
        match (role, self.server.advertise_url.as_deref()) {
            (NodeRole::Worker, None | Some("")) => errors.push(
                "server.role 'worker' requires server.advertise_url: the address api replicas \
                 use to reach this worker's MCP and agent routes (e.g. http://${POD_IP}:8080)"
                    .to_owned(),
            ),
            (_, Some(url)) if !url.is_empty() && !is_http_url(url) => errors.push(format!(
                "server.advertise_url must be an absolute http(s) URL (got '{url}')"
            )),
            _ => {},
        }
    }

    pub(crate) fn validate_shutdown(&self, errors: &mut Vec<String>) {
        errors.extend(self.server.shutdown.validation_errors());
    }
}

fn is_http_url(url: &str) -> bool {
    url.strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
}
