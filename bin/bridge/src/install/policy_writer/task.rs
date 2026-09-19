//! Task Scheduler registration for the policy writer: a SYSTEM task with no
//! triggers that authenticated users may start and nobody but an
//! administrator may change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use std::io;
use std::path::Path;
use std::process::Command;

use base64::Engine as _;

use super::{TASK_SDDL, render_task_xml, task_name};
use crate::winproc;

// Why: `schtasks /Create` cannot set the task's security descriptor, and a
// task an administrator registers is invisible to every other account
// unless its descriptor admits them. The Task Scheduler COM API takes the
// descriptor with the registration; PowerShell is the one caller of it that
// ships with Windows. `-EncodedCommand` carries the script byte for byte, so
// no path or descriptor is ever re-parsed by a shell.
pub(super) fn register(binary: &Path, spool_root: &Path) -> io::Result<()> {
    let xml = render_task_xml(binary, spool_root);
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n\
         $service = New-Object -ComObject Schedule.Service\n\
         $service.Connect()\n\
         $folder = $service.GetFolder('\\')\n\
         $xml = [System.Text.Encoding]::Unicode.GetString([System.Convert]::FromBase64String('{xml_b64}'))\n\
         $null = $folder.RegisterTask('{name}', $xml, 6, $null, $null, 5, '{sddl}')\n",
        xml_b64 = base64::engine::general_purpose::STANDARD.encode(utf16le(&xml)),
        name = task_name(),
        sddl = TASK_SDDL,
    );
    run_powershell(&script)?;
    verify_registered(binary)
}

pub(super) fn remove() -> io::Result<()> {
    let output = winproc::no_window(&mut Command::new(winproc::system32("schtasks.exe")))
        .args(["/Delete", "/TN", &task_name(), "/F"])
        .output()?;
    if output.status.success() || !exists()? {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "schtasks /Delete exited with {}: {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

pub(super) fn exists() -> io::Result<bool> {
    Ok(query_xml()?.is_some())
}

pub(super) fn run() -> io::Result<()> {
    let output = winproc::no_window(&mut Command::new(winproc::system32("schtasks.exe")))
        .args(["/Run", "/TN", &task_name()])
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "schtasks /Run exited with {}: {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

// Why: the registration is verified from what the scheduler holds, not from
// the exit code of the call that made it: the principal must be SYSTEM and
// the command the admin-owned copy, or a later request would run the wrong
// binary with the wrong rights.
pub(super) fn verify_registered(binary: &Path) -> io::Result<()> {
    let Some(xml) = query_xml()? else {
        return Err(io::Error::other(format!(
            "task {} is not registered",
            task_name()
        )));
    };
    let command = binary.display().to_string();
    if !xml.contains("<UserId>S-1-5-18</UserId>") {
        return Err(io::Error::other(format!(
            "task {} does not run as SYSTEM",
            task_name()
        )));
    }
    if !xml.contains(&format!("<Command>{command}</Command>")) {
        return Err(io::Error::other(format!(
            "task {} does not run {command}",
            task_name()
        )));
    }
    Ok(())
}

fn query_xml() -> io::Result<Option<String>> {
    let output = winproc::no_window(&mut Command::new(winproc::system32("schtasks.exe")))
        .args(["/Query", "/TN", &task_name(), "/XML"])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
}

fn run_powershell(script: &str) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16le(script));
    let output = winproc::no_window(&mut Command::new(winproc::powershell_exe()))
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ])
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "task registration exited with {}: {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn utf16le(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
