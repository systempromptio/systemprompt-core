//! Long-running work an extension contributes and the composition root owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_traits::CancellationToken;

/// Work that runs for the life of the app, such as following the active
/// account.
///
/// Held as `dyn`, which is why the trait uses `async_trait`. The composition
/// root spawns `run` as an owned task under `name` and cancels it at
/// shutdown; `run` returns once `cancel` fires. An extension never spawns its
/// own task.
#[async_trait]
pub trait BackgroundService: Send + Sync {
    fn name(&self) -> &'static str;

    async fn run(&self, cancel: CancellationToken);
}
