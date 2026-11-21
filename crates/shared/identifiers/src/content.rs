//! Content management identifiers.
//!
//! [`SkillName`] is checked non-empty. [`SkillId`] is the catalog slug a
//! manifest, an artifact and an authz entity ref all carry, and the directory
//! name of the skill under the services tree, so it is checked as one path
//! segment: no separator, no null byte, never `.` or `..`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(SkillId, checked, |value| {
    crate::macros::validate_path_segment("SkillId", value)
});
crate::define_id!(SkillName, checked, |value| {
    crate::macros::validate_non_empty("SkillName", value)
});
crate::define_id!(SourceId, schema);
crate::define_id!(CategoryId, schema);
crate::define_id!(ContentId, generate, schema);
crate::define_id!(TagId, schema);
crate::define_id!(FileId, uuid);
