//! GUI handlers for log-directory access and diagnostic-bundle export.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::json;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use std::sync::Arc;

use crate::gui::error::GuiError;
use crate::gui::events::ReplyId;
use crate::gui::{GuiApp, emit};
use crate::i18n;
use crate::wire::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_open_log_directory(app: &GuiApp, reply_to: ReplyId) {
    let result = crate::obs::log_dir().map_or_else(
        || {
            Err(BridgeError::new(
                ErrorScope::Internal,
                ErrorCode::NotFound,
                "log directory unavailable on this platform",
            ))
        },
        |dir| {
            if let Err(e) = fs::create_dir_all(&dir) {
                let msg = format!("create log dir failed: {e}");
                app.append_log_error(&msg);
                Err(BridgeError::new(
                    ErrorScope::Internal,
                    ErrorCode::Internal,
                    msg,
                ))
            } else if let Err(e) = opener::reveal(&dir) {
                let msg = format!("reveal log dir failed: {e}");
                app.append_log_error(&msg);
                Err(BridgeError::new(
                    ErrorScope::Internal,
                    ErrorCode::Internal,
                    msg,
                ))
            } else {
                app.append_log(format!("opened log folder {}", dir.display()));
                Ok(json!({ "path": dir.display().to_string() }))
            }
        },
    );
    finish(app, result, reply_to);
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_export_diagnostic_bundle(app: &GuiApp, reply_to: ReplyId) {
    let result = build_bundle(&app.ctx).map_err(|e| {
        let msg = format!("export diagnostic bundle failed: {e}");
        app.append_log_error(&msg);
        BridgeError::new(ErrorScope::Internal, ErrorCode::Internal, msg)
    });
    if let Ok(path) = result.as_ref() {
        app.append_log(format!("diagnostic bundle written to {}", path.display()));
        // Why: the bundle exists and its path is the answer; a file manager
        // that will not open is worth a log line, not a failed export.
        if let Err(e) = opener::reveal(path) {
            app.append_log_error(format!(
                "bundle saved at {}, but reveal failed: {e}",
                path.display()
            ));
        }
    }
    let value = result.map(|p| json!({ "path": p.display().to_string() }));
    finish(app, value, reply_to);
}

// Why: the running proxy holds the secret it started with, so a reset only
// takes effect in a fresh process. The relaunch is the same one an update
// uses; when it cannot spawn, the reset still stands and the operator restarts
// by hand.
#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_reset_proxy_secret(app: &GuiApp, reply_to: ReplyId) {
    let result = crate::proxy::secret::reset()
        .map(|(_, path)| {
            app.append_log(format!(
                "local proxy secret reset at {}; restarting the bridge, then repair each agent \
                 so it learns the new secret",
                path.display()
            ));
            json!({ "path": path.display().to_string() })
        })
        .map_err(|e| {
            let msg = format!("reset local proxy secret failed: {e}");
            app.append_log_error(&msg);
            BridgeError::new(ErrorScope::Internal, ErrorCode::Internal, msg)
        });
    let reset_ok = result.is_ok();
    finish(app, result, reply_to);
    if reset_ok {
        app.proxy
            .send_event(crate::gui::events::UiEvent::UpdateRestartRequested);
    }
}

// Why: the UAC prompt blocks until the user answers it, so the repair runs off
// the event loop and reports back the way a profile install does.
#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_config_dir_repair_requested(app: &GuiApp, reply_to: ReplyId) {
    app.append_log(i18n::t("config-dir-repair-started"));
    let proxy = app.proxy.clone();
    app.ctx.spawn(async move {
        let result = match tokio::task::spawn_blocking(repair_config_dir).await {
            Ok(r) => r.map_err(|e| Arc::new(GuiError::Io(e))),
            Err(join_err) => Err(Arc::new(GuiError::Io(io::Error::other(format!(
                "config dir repair task join: {join_err}"
            ))))),
        };
        proxy.send_event(crate::gui::events::UiEvent::ConfigDirRepairFinished { result, reply_to });
    });
}

pub(crate) fn on_config_dir_repair_finished(
    app: &mut GuiApp,
    result: Result<String, Arc<GuiError>>,
    reply_to: ReplyId,
) {
    let result = match result {
        Ok(path) => {
            app.append_log(i18n::t_args("config-dir-repaired", &[("path", &path)]));
            Ok(json!({ "path": path }))
        },
        Err(e) => {
            let msg = i18n::t_args("config-dir-repair-failed", &[("error", &e.to_string())]);
            app.append_log_error(&msg);
            let code = match e.as_ref() {
                GuiError::Io(io) if io.kind() == io::ErrorKind::PermissionDenied => {
                    ErrorCode::Unauthorized
                },
                _ => ErrorCode::Internal,
            };
            Err(BridgeError::new(ErrorScope::Identity, code, msg))
        },
    };
    app.state.reload();
    app.refresh_ui();
    finish(app, result, reply_to);
}

#[cfg(target_os = "windows")]
fn repair_config_dir() -> io::Result<String> {
    crate::windows_acl::repair_config_dir_elevated().map(|dir| dir.display().to_string())
}

#[cfg(not(target_os = "windows"))]
fn repair_config_dir() -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the configuration directory carries no Windows access control list to repair",
    ))
}

fn build_bundle(ctx: &crate::context::BridgeContext) -> io::Result<PathBuf> {
    let log_dir = crate::obs::log_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "log dir unavailable"))?;
    let dest_dir = crate::basedirs::desktop_dir()
        .or_else(crate::basedirs::home_dir)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home dir"))?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let zip_path = dest_dir.join(format!(
        "{}-diagnostics-{ts}.zip",
        crate::brand::brand().binary_name
    ));

    let file = fs::File::create(&zip_path)?;
    let mut zip = ZipWriter::new(file);
    let opts: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let include = name.starts_with("bridge.")
                || name.starts_with("bridge-crash-")
                || name == "activity.jsonl"
                || name == "activity.jsonl.1";
            if !include {
                continue;
            }
            if path.is_file() {
                add_file(&mut zip, &path, name, opts)?;
            }
        }
    }

    zip.start_file("diagnostics.txt", opts)?;
    zip.write_all(crate::buildinfo::render().as_bytes())?;

    zip.start_file("state.txt", opts)?;
    zip.write_all(crate::diagnostics_state::render(ctx).as_bytes())?;

    zip.start_file("registry.txt", opts)?;
    zip.write_all(crate::diagnostics_state::registry::render().as_bytes())?;

    for (name, path) in state_files() {
        if let Ok(bytes) = fs::read(&path) {
            zip.start_file(name, opts)?;
            zip.write_all(&bytes)?;
        }
    }

    if let Some(yaml) = crate::config::redaction::redacted_config() {
        zip.start_file("config.redacted.toml", opts)?;
        zip.write_all(yaml.as_bytes())?;
    }

    zip.finish()?;
    Ok(zip_path)
}

// Why: these hold no secret, and a bundle that carries them verbatim lets the
// port record, install identity and sync checkpoint be compared across
// bundles without asking the user for another export.
fn state_files() -> Vec<(&'static str, PathBuf)> {
    let mut files = Vec::new();
    if let Some(path) = crate::proxy::portfile::portfile_path() {
        files.push(("bridge-proxy.json", path));
    }
    if let Some(path) = crate::proxy::identity::install_id_path() {
        files.push(("bridge-install.id", path));
    }
    if let Some(meta) = crate::config::paths::bridge_metadata_dir() {
        files.push((
            "last-sync.json",
            meta.join(crate::config::paths::LAST_SYNC_SENTINEL),
        ));
    }
    files
}

fn add_file(
    zip: &mut ZipWriter<fs::File>,
    path: &Path,
    name: &str,
    opts: SimpleFileOptions,
) -> io::Result<()> {
    zip.start_file(name, opts)?;
    let buf = fs::read(path)?;
    zip.write_all(&buf)?;
    Ok(())
}

fn finish(app: &GuiApp, result: Result<serde_json::Value, BridgeError>, reply_to: ReplyId) {
    let Some(id) = reply_to else {
        if let Err(err) = result {
            emit::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            emit::emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    emit::send_reply_payload(app, id, &payload);
}
