use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::json;
use zip::ZipWriter;
use zip::write::FileOptions;

use crate::gui::events::ReplyId;
use crate::gui::ipc::{BridgeError, ErrorCode, ErrorScope, IpcReplyPayload};
use crate::gui::{GuiApp, ipc_runtime};

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_open_log_directory(app: &mut GuiApp, reply_to: ReplyId) {
    let result = match crate::obs::log_dir() {
        Some(dir) => {
            if let Err(e) = fs::create_dir_all(&dir) {
                let msg = format!("create log dir failed: {e}");
                app.append_log(&msg);
                Err(BridgeError::new(
                    ErrorScope::Internal,
                    ErrorCode::Internal,
                    msg,
                ))
            } else if let Err(e) = opener::reveal(&dir) {
                let msg = format!("reveal log dir failed: {e}");
                app.append_log(&msg);
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
        None => Err(BridgeError::new(
            ErrorScope::Internal,
            ErrorCode::NotFound,
            "log directory unavailable on this platform",
        )),
    };
    finish(app, result, reply_to);
}

#[tracing::instrument(level = "info", skip(app))]
pub(crate) fn on_export_diagnostic_bundle(app: &mut GuiApp, reply_to: ReplyId) {
    let result = build_bundle().map_err(|e| {
        let msg = format!("export diagnostic bundle failed: {e}");
        app.append_log(&msg);
        BridgeError::new(ErrorScope::Internal, ErrorCode::Internal, msg)
    });
    if let Ok(path) = result.as_ref() {
        app.append_log(format!("diagnostic bundle written to {}", path.display()));
        let _ = opener::reveal(path);
    }
    let value = result.map(|p| json!({ "path": p.display().to_string() }));
    finish(app, value, reply_to);
}

fn build_bundle() -> io::Result<PathBuf> {
    let log_dir = crate::obs::log_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "log dir unavailable"))?;
    let dest_dir = dirs::desktop_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home dir"))?;
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let zip_path = dest_dir.join(format!("systemprompt-bridge-diagnostics-{ts}.zip"));

    let file = fs::File::create(&zip_path)?;
    let mut zip = ZipWriter::new(file);
    let opts: FileOptions =
        FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

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
    zip.write_all(crate::cli::diagnostics::render().as_bytes())?;

    if let Some(yaml) = crate::config::redaction::redacted_config() {
        zip.start_file("config.redacted.toml", opts)?;
        zip.write_all(yaml.as_bytes())?;
    }

    zip.finish()?;
    Ok(zip_path)
}

fn add_file(
    zip: &mut ZipWriter<fs::File>,
    path: &Path,
    name: &str,
    opts: FileOptions,
) -> io::Result<()> {
    zip.start_file(name, opts)?;
    let mut f = fs::File::open(path)?;
    let mut buf = Vec::with_capacity(8 * 1024);
    f.read_to_end(&mut buf)?;
    zip.write_all(&buf)?;
    Ok(())
}

fn finish(app: &GuiApp, result: Result<serde_json::Value, BridgeError>, reply_to: ReplyId) {
    let Some(id) = reply_to else {
        if let Err(err) = result {
            ipc_runtime::emit_error(app, &err);
        }
        return;
    };
    let payload = match result {
        Ok(v) => IpcReplyPayload::ok(v),
        Err(err) => {
            ipc_runtime::emit_error(app, &err);
            IpcReplyPayload::err(err)
        },
    };
    ipc_runtime::send_reply_payload(app, id, &payload);
}
