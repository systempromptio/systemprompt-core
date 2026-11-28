# systemprompt-machine

[![Crates.io](https://img.shields.io/crates/v/systemprompt-machine.svg?style=flat-square)](https://crates.io/crates/systemprompt-machine)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-machine?style=flat-square)](https://docs.rs/systemprompt-machine)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Machine-level install, device-management snippets, policy writer and elevation for systemprompt.io devices.

Scheduler entries (Task Scheduler, launchd agents, systemd user units), elevation (UAC, the macOS authorization dialog, pkexec) and the locations a managed configuration is read from, each one typed contract with a Windows, a macOS and a Linux implementation in the `platform` module.

**Layer**: Infra. Device infrastructure consumed by the device application crate; it depends on no server crate above the shared layer. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
