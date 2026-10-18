//! Notices shown beside a profile install: elevation and OS approval.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(super) fn needs_elevation_notice(
    #[cfg_attr(
        not(target_os = "windows"),
        expect(
            unused_variables,
            reason = "only the registry-backed host needs elevation"
        )
    )]
    host: &dyn crate::integration::HostApp,
) -> bool {
    #[cfg(target_os = "windows")]
    {
        host.config_format() == crate::integration::ConfigFormat::Reg
            && !crate::winproc::is_elevated()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub(super) fn manual_approval_notice(host: &dyn crate::integration::HostApp) -> Option<String> {
    (cfg!(target_os = "macos") && host.config_format() == crate::integration::ConfigFormat::Plist)
        .then(|| {
            format!(
                "macOS holds the profile until you approve it: open System Settings › General › \
             Device Management, select the {} profile and click Install. Until then this host \
             correctly reports no profile.",
                host.display_name()
            )
        })
}
