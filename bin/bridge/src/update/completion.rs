//! Delivering an install's outcome exactly once.
//!
//! The install runs on a background task, and the GUI's Downloading state ends
//! only when an outcome arrives. A [`Completion`] dropped without
//! [`Completion::finish`], because the task panicked or was cancelled,
//! delivers [`UpdateError::InstallTaskEnded`] instead, so the state always
//! moves on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::UpdateError;

type Deliver<T, E> = Box<dyn FnOnce(Result<T, E>) + Send>;

/// The one-shot outcome channel of a background install.
pub struct Completion<T, E: From<UpdateError>> {
    deliver: Option<Deliver<T, E>>,
}

impl<T, E: From<UpdateError>> Completion<T, E> {
    #[must_use]
    pub fn new(deliver: impl FnOnce(Result<T, E>) + Send + 'static) -> Self {
        Self {
            deliver: Some(Box::new(deliver)),
        }
    }

    pub fn finish(mut self, result: Result<T, E>) {
        if let Some(deliver) = self.deliver.take() {
            deliver(result);
        }
    }
}

impl<T, E: From<UpdateError>> Drop for Completion<T, E> {
    fn drop(&mut self) {
        if let Some(deliver) = self.deliver.take() {
            tracing::error!("update: the install task ended before reporting an outcome");
            deliver(Err(E::from(UpdateError::InstallTaskEnded)));
        }
    }
}

impl<T, E: From<UpdateError>> std::fmt::Debug for Completion<T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Completion")
            .field("pending", &self.deliver.is_some())
            .finish()
    }
}
