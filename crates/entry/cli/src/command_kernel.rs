//! The kernel a command resolves state through: the application context's
//! kernel state for a database command, or a detached kernel over the
//! profile-level seeds (the admitted schedule and open views, the services
//! view and the profile's `Arc<AppPaths>`) for a command that needs no
//! database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_extension::AdmittedSchedule;
use systemprompt_extension::cli::AdmittedViews;
use systemprompt_loader::BootContext;
use systemprompt_runtime::Kernel;
use systemprompt_traits::{AppContext as _, BackgroundTasks, Dependencies};

use crate::context::CommandContext;

impl CommandContext {
    #[must_use]
    pub fn with_detached_kernel(self) -> Self {
        let mut seed = self.command_seed(Dependencies::new());
        if let Some(admitted) = &self.admitted {
            match AdmittedSchedule::of(admitted.built()) {
                Ok(schedule) => seed.insert(schedule),
                Err(error) => {
                    tracing::error!(%error, "admitted schedule refused; no job is readable");
                },
            }
            let views = AdmittedViews::of(admitted);
            seed.insert(views.migrations);
            seed.insert(views.configs);
            seed.insert(views.assets);
        }
        if let Some(paths) = self.detached_paths() {
            seed.insert(Arc::new(paths));
        }
        let kernel = Kernel::detached(BackgroundTasks::new(), seed);
        self.with_kernel(Arc::new(kernel))
    }

    fn detached_paths(&self) -> Option<AppPaths> {
        let profile = self.loaded_profile().ok()?.profile();
        let root = self
            .boot()
            .ok()
            .and_then(BootContext::services_root)
            .map(|root| root.path.clone());
        AppPaths::from_profile(&profile.paths, profile.path_resolution(), root.as_deref())
            .inspect_err(
                |error| tracing::warn!(%error, "profile paths unresolved; no paths are readable"),
            )
            .ok()
    }

    fn command_seed(&self, base: Dependencies) -> Dependencies {
        let mut seed = base.with(self.interrupt_shield().clone());
        if let Ok(profile) = self.loaded_profile() {
            seed.insert_sealed(Arc::clone(profile));
        }
        if let Some(services) = self.services() {
            seed.insert(services.clone());
        }
        seed
    }

    pub(crate) async fn command_kernel(&self) -> anyhow::Result<&Arc<Kernel>> {
        self.kernel
            .get_or_try_init(|| async {
                let app = self.app_context().await?;
                let state = self.command_seed(app.kernel().state().clone());
                Ok(Arc::new(Kernel::new(
                    app.database_handle(),
                    app.background_tasks().clone(),
                    state,
                )))
            })
            .await
    }
}
