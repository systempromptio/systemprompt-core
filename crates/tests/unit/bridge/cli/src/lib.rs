#![allow(clippy::all)]
#[cfg(test)]
mod args;
#[cfg(all(test, unix))]
mod comms_drain;
#[cfg(test)]
mod credential_helper;
#[cfg(test)]
mod diagnostics;
#[cfg(test)]
mod doctor;
#[cfg(test)]
mod doctor_auth;
#[cfg(test)]
mod doctor_cowork;
#[cfg(test)]
mod doctor_filesystem;
#[cfg(test)]
mod proxy_command;
#[cfg(test)]
mod doctor_marketplace;
#[cfg(test)]
mod login_helpers;
#[cfg(test)]
mod context_probe;
#[cfg(test)]
mod proxy_command_roles;
#[cfg(test)]
mod update_command;
