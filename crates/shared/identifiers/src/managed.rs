//! Stable identities for managed authoring, revisions and publication.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ManagedSourceId, generate, schema);
crate::define_id!(SourceSnapshotId, generate, schema);
crate::define_id!(ManagedResourceId, generate, schema);
crate::define_id!(ResourceRevisionId, generate, schema);
crate::define_id!(PublicationId, generate, schema);
