//! Tenant identifier.
//!
//! `organization_uuid` is the only derivation of the organisation UUID hosts
//! are configured with: a `local_<uuid>` or bare-UUID tenant id yields that
//! UUID, any other id a stable v5 UUID of its text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(TenantId, checked, |value| {
    crate::macros::validate_non_empty("TenantId", value)
});

impl TenantId {
    #[must_use]
    pub fn organization_uuid(&self) -> String {
        let raw = self.as_str();
        let suffix = raw.strip_prefix("local_").unwrap_or(raw);
        if let Ok(parsed) = uuid::Uuid::parse_str(suffix) {
            return parsed.to_string();
        }
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, raw.as_bytes()).to_string()
    }
}
