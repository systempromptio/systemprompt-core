//! The files member's composed provides: its loaded configuration
//! (`Arc<FilesConfig>`), read from `services/config/files.yaml` under the
//! kernel's paths, validated and with the storage tree created, and the open
//! view of it the API host reads (`Arc<FilesConfigView>`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_traits::FilesConfigView;

use crate::FilesConfig;

pub(crate) fn files_config(r: &Resolved<'_>) -> Result<Arc<FilesConfig>, ProvideError> {
    let paths = r.get::<Arc<AppPaths>>()?;
    let failed = |e: crate::FilesError| ProvideError::Failed(Box::new(e));
    let config = FilesConfig::from_app_paths(&paths).map_err(failed)?;
    config.validate().map_err(failed)?;
    config.ensure_storage_structure().map_err(failed)?;
    Ok(Arc::new(config))
}

pub(crate) fn files_config_view(r: &Resolved<'_>) -> Result<Arc<FilesConfigView>, ProvideError> {
    let config = r.get::<Arc<FilesConfig>>()?;
    Ok(Arc::new(FilesConfigView {
        url_prefix: config.url_prefix().to_owned(),
        files_root: config.files(),
        cache_control: config.cache_control().map(str::to_owned),
        documents_enabled: config.documents_config().is_enabled(),
    }))
}
