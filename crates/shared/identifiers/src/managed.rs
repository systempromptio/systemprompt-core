//! Stable identities for managed authoring, revisions and publication.

crate::define_id!(ManagedSourceId, generate, schema);
crate::define_id!(SourceSnapshotId, generate, schema);
crate::define_id!(ManagedResourceId, generate, schema);
crate::define_id!(ResourceRevisionId, generate, schema);
crate::define_id!(PublicationId, generate, schema);
