//! `server` replica-identity and capacity checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::Profile;

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
}
