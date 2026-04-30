use super::{InstallError, ScheduleEmit};
use crate::schedule::{self, Os};
use std::fs;
use std::path::{Path, PathBuf};

pub fn emit_schedule(schedule_os: Os, binary: &Path) -> Result<ScheduleEmit, InstallError> {
    let filename = schedule::template_filename(schedule_os);
    let content = schedule::template(schedule_os, binary);
    let out = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(filename);
    fs::write(&out, content).map_err(|e| InstallError::Schedule {
        path: out.display().to_string(),
        source: e,
    })?;
    Ok(ScheduleEmit {
        os: schedule_os,
        path: out,
        install_hint: schedule::install_hint(schedule_os).to_string(),
    })
}
