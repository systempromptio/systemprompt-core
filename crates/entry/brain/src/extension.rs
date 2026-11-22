//! [`BrainHttpExtension`] (`brain-http`): the brain route sets.
//!
//! With `http` it contributes two route sets in the content-and-misc role
//! group under the `brain` rate-limit scope and budget with the default body
//! limit, both with a user request context: `/api/v1/brain` behind the user
//! gate and `/api/v1/admin/brain` behind the admin gate. Each shape equals
//! the reference composition's conferral for `brain-http` at its base.
//!
//! The handlers read the brain handle the `brain` member's slot build
//! returns (`Arc<BrainHandleSlot>`) and the recall it provides
//! (`Arc<dyn MemoryRecall>`), which this member `requires`, the kernel's
//! brain profile, the governance role directory a caller's current roles are
//! read from, and the subject providers the simulator resolves a default
//! `project` through. While the profile disables the brain the handle is
//! unbound and every route answers 404 `brain_disabled`.
//!
//! | profile disables | user | admin |
//! |---|---|---|
//! | `brain-http` | 404 (not mounted) | 404 (not mounted) |
//! | `brain` | boot refused (unmet `Arc<BrainHandleSlot>`) | |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const BRAIN_HTTP_EXTENSION: &str = "brain-http";

/// The member that contributes the brain route sets.
#[derive(Debug, Clone, Copy, Default)]
pub struct BrainHttpExtension;

impl Extension for BrainHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: BRAIN_HTTP_EXTENSION,
            name: "Brain HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(feature = "http"),
        expect(
            unused_variables,
            reason = "the member contributes only routes, which the http feature compiles"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        routes::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod routes {
    use std::sync::Arc;

    use systemprompt_brain::BrainHandleSlot;
    use systemprompt_extension::brain::MemoryRecall;
    use systemprompt_extension::prelude::*;

    use crate::http::{admin_routes, user_routes};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<BrainHandleSlot>>()
            .requires::<Arc<dyn MemoryRecall>>()
            .routes(user_routes())
            .routes(admin_routes());
    }
}
