//! Replica identity and the keys replicas coordinate on.
//!
//! `LeaseKey` names a resource one replica at a time may own for a bounded
//! time; `AdvisoryLockKey` names a session-scoped cluster-wide lock.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(InstanceId, checked, |value| {
    crate::macros::validate_non_empty("InstanceId", value)
});
crate::define_id!(LeaseKey, checked, |value| {
    crate::macros::validate_non_empty("LeaseKey", value)
});
crate::define_id!(AdvisoryLockKey, checked, |value| {
    crate::macros::validate_non_empty("AdvisoryLockKey", value)
});
