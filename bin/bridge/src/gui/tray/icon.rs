//! Tray icon bitmaps: the platform base image and the alert variant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tray_icon::Icon;

use super::super::error::{GuiError, GuiResult};

#[cfg(target_os = "macos")]
fn tray_image() -> Result<image::RgbaImage, image::ImageError> {
    let assets = crate::brand::brand().assets;
    Ok(image::load_from_memory(assets.tray_icon_png)?.to_rgba8())
}

#[cfg(target_os = "windows")]
fn tray_image() -> Result<image::RgbaImage, image::ImageError> {
    let assets = crate::brand::brand().assets;
    let reader = image::codecs::ico::IcoDecoder::new(std::io::Cursor::new(assets.app_icon_ico));
    match reader {
        Ok(decoder) => Ok(image::DynamicImage::from_decoder(decoder)?.to_rgba8()),
        Err(e) => {
            tracing::warn!(error = %e, "app icon ICO undecodable; falling back to the window icon");
            Ok(image::load_from_memory(assets.window_icon_png)?.to_rgba8())
        },
    }
}

pub(super) fn decode_icon() -> GuiResult<Icon> {
    let img = tray_image()?;
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).map_err(GuiError::from)
}

pub(super) fn decode_alert_icon() -> GuiResult<Icon> {
    let mut img = tray_image()?;
    let (w, h) = img.dimensions();
    let dot_radius = (w.min(h) / 4).max(3);
    let cx = w.saturating_sub(dot_radius).saturating_sub(1);
    let cy = h.saturating_sub(dot_radius).saturating_sub(1);
    for y in 0..h {
        for x in 0..w {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            if dx * dx + dy * dy <= (dot_radius as i32).pow(2) {
                img.put_pixel(x, y, image::Rgba([220, 38, 38, 255]));
            }
        }
    }
    Icon::from_rgba(img.into_raw(), w, h).map_err(GuiError::from)
}
