#![allow(clippy::all)]

#[cfg(test)]
mod bootstrap;
#[cfg(test)]
mod builders;
#[cfg(test)]
mod egress;
#[cfg(test)]
mod elevated_protocol;
#[cfg(test)]
mod elevation_script;
#[cfg(test)]
mod linux_managed_settings;
#[cfg(test)]
mod linux_settings;
#[cfg(test)]
mod managed_file_writes;
#[cfg(test)]
mod managed_settings;
#[cfg(test)]
mod mdm_snippet;
#[cfg(all(test, unix))]
mod model_picker;
#[cfg(test)]
mod policy;
#[cfg(test)]
mod pubkey;
#[cfg(test)]
mod schedule;
#[cfg(test)]
mod summary;
#[cfg(test)]
mod uninstall;
#[cfg(test)]
mod user_alert;
