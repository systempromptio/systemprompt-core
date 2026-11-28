//! The build the runner signs in and runs as: the harness composition with
//! its registered extensions, an in-memory sealer and settings, under one
//! fresh directory.

use std::path::Path;
use std::sync::Arc;

use systemprompt_device::secure::MemorySealer;
use systemprompt_harness::command::{HeadlessHost, MemorySettings};
use systemprompt_harness::extension::HostInfo;
use systemprompt_harness::members::REGISTERED;

pub const PRODUCT: &str = "golden-runner";
pub const PROJECT_DIR: &str = ".harness";

pub fn host(dir: &Path) -> HeadlessHost {
    HeadlessHost {
        info: HostInfo {
            product: PRODUCT,
            display_name: "Golden Runner",
            version: "0.0.0",
        },
        project_dir: PROJECT_DIR.to_owned(),
        device_name: "Golden runner".to_owned(),
        data_root: dir.join("root"),
        home: dir.join("home"),
        documents: None,
        sealer: Arc::new(MemorySealer::for_tests()),
        settings: Arc::new(MemorySettings::default()),
        registrations: REGISTERED.to_vec(),
        verdicts_enabled: true,
        search_path: None,
        path_ext: None,
    }
}
