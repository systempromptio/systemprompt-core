//! Device record wire shapes: what a signed-in device reports about itself
//! and what the admin device list shows.
//!
//! A device record belongs to one user. [`DevicePlatform`] names the operating
//! system family it runs; a record written before a device reported one reads
//! [`DevicePlatform::Unknown`]. [`AppIdentifier`] is the reverse-DNS name an
//! app is published under: an Apple bundle identifier or an Android package
//! name. A device sends [`DeviceClientInfo`] when it registers a push token
//! (see [`crate::notifications::push`]); [`AdminDeviceRow`] is one row of the
//! admin device list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod app;
mod platform;
mod record;

pub use app::{AppIdentifier, AppIdentifierError};
pub use platform::{DevicePlatform, UnknownDevicePlatform};
pub use record::{AdminDeviceRow, DeviceClientInfo};
