//! Typed identifiers for marketplace catalog entries.
//!
//! [`MarketplaceRuleId`] is the slug of a marketplace rule (a governance
//! instruction shipped to coding agents), distinct from the authz
//! [`RuleId`](crate::RuleId). [`LibraryArtifactId`] names a library artifact
//! a plugin bundles. All three entry ids are checked non-empty.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(MarketplaceId, schema);
crate::define_id!(MarketplaceRuleId, checked, |value| {
    crate::macros::validate_non_empty("MarketplaceRuleId", value)
});
crate::define_id!(RuleName, checked, |value| {
    crate::macros::validate_non_empty("RuleName", value)
});
crate::define_id!(LibraryArtifactId, checked, |value| {
    crate::macros::validate_non_empty("LibraryArtifactId", value)
});
