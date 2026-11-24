//! The write halves of the kernel state's late-bound cells.
//!
//! [`base_kernel_state`](super::kernel_state::base_kernel_state) and
//! [`kernel_state`](super::kernel_state::kernel_state) seed only each cell's
//! read half and return its bind half here, outside the extension state. The
//! host consumes each binder in the one kernel function that binds the cell;
//! the API's binders wait in [`PendingBinders`] on the context until the
//! route build takes them, once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, PoisonError};

use systemprompt_governance::authz::AuthzHookBinder;
use systemprompt_governance::policy::secrets::SecretScannerBinder;
use systemprompt_models::LateBinder;
use systemprompt_models::a2a::dispatch::AgentDispatchBinder;
use systemprompt_traits::MessagingDispatcher;

/// The write halves of the late-bound cells every host seeds: kept by the
/// host outside the extension state and consumed by the one kernel function
/// that binds each cell.
#[derive(Debug)]
pub struct KernelBinders {
    pub secret_scanner: SecretScannerBinder,
    pub authz_hook: AuthzHookBinder,
}

/// The write halves of the late-bound cells only the API seeds, beside the
/// [`KernelBinders`] every host keeps.
#[derive(Debug)]
pub struct ApiKernelBinders {
    pub base: KernelBinders,
    pub late: ApiLateBinders,
}

/// The write halves of the cells the API binds while it builds its routes:
/// the agent dispatch and the messaging dispatcher, both of which hold the
/// built context.
#[derive(Debug)]
pub struct ApiLateBinders {
    pub agent_dispatch: AgentDispatchBinder,
    pub messaging: LateBinder<dyn MessagingDispatcher>,
}

/// The API's [`ApiLateBinders`] between the context build and the route
/// build that binds them: taken once, so a second take is `None`.
#[derive(Clone, Default)]
pub struct PendingBinders(Arc<Mutex<Option<ApiLateBinders>>>);

impl PendingBinders {
    #[must_use]
    pub fn holding(binders: ApiLateBinders) -> Self {
        Self(Arc::new(Mutex::new(Some(binders))))
    }

    #[must_use]
    pub fn take(&self) -> Option<ApiLateBinders> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

impl std::fmt::Debug for PendingBinders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pending = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some();
        f.debug_struct("PendingBinders")
            .field("pending", &pending)
            .finish()
    }
}
