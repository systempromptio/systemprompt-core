fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app-icon.ico");
        res.set("FileDescription", "Systemprompt Bridge");
        res.set("ProductName", "Systemprompt Bridge");
        res.set("CompanyName", "Systemprompt");
        res.set(
            "LegalCopyright",
            "Copyright (C) 2026 Edward Burton. BUSL-1.1.",
        );
        res.set("OriginalFilename", "systemprompt-bridge.exe");
        res.set("InternalName", "systemprompt-bridge");
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
            res.set_toolkit_path("/usr/x86_64-w64-mingw32/bin");
            res.set_windres_path("x86_64-w64-mingw32-windres");
            res.set_ar_path("x86_64-w64-mingw32-ar");
        }
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=winresource compile failed: {e}");
        }
        println!("cargo:rerun-if-changed=assets/app-icon.ico");
        println!("cargo:rerun-if-changed=build.rs");
    }
}
