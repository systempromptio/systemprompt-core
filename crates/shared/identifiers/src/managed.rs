//! Stable identities for managed authoring, revisions and publication.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ManagedSourceId, generate, schema);
crate::define_id!(SourceSnapshotId, generate, schema);
crate::define_id!(ManagedResourceId, generate, schema);
crate::define_id!(ResourceRevisionId, generate, schema);
crate::define_id!(PublicationReviewId, generate, schema);
crate::define_id!(PublicationId, generate, schema);
crate::define_id!(ManagedReconciliationId, generate, schema);
crate::define_id!(WithdrawalProposalId, generate, schema);
crate::define_id!(DistributionId, generate, schema);
crate::define_id!(InstallationReceiptId, generate, schema);
crate::define_id!(InvocationAttributionId, generate, schema);
crate::define_id!(ResourceInvocationId, schema);

crate::define_id!(ConsumerInstallationId, generate, schema);
crate::define_id!(InstallationSessionBindingId, generate, schema);
crate::define_id!(NativeSessionId, schema);
crate::define_id!(InventoryEntryId, generate, schema);
crate::define_id!(AnalyticsChangeId, generate, schema);
crate::define_id!(AnalyticsFactId, schema);
crate::define_id!(AnalyticsSnapshotJobId, generate, schema);
crate::define_id!(AnalyticsWorkerId, generate, schema);
crate::define_id!(DependencyVerificationId, generate, schema);
