//! Extension identifier: the `ExtensionMetadata` id an extension registers
//! under, keyed in `extension_migrations.extension_id`, the profile's
//! disabled-extension list and the `infra db` `--extension` arguments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ExtensionId, checked, |value| {
    crate::macros::validate_non_empty("ExtensionId", value)
});
