//! Delivery channels the notifications crate ships: the AG-UI stream of the
//! recipient, and the Apple and Google push channels an instance with push
//! credentials contributes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agui;
pub mod apns;
pub mod fcm;
pub mod push;

pub use agui::AguiChannel;
pub use apns::ApnsChannel;
pub use fcm::FcmChannel;
pub use push::PushChannel;
