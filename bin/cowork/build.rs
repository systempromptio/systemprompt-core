fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app-icon.ico");
        res.set("FileDescription", "Systemprompt Cowork");
        res.set("ProductName", "Systemprompt Cowork");
        res.set("CompanyName", "Systemprompt");
        res.set(
            "LegalCopyright",
            "Copyright (C) 2026 Edward Burton. BUSL-1.1.",
        );
        res.set("OriginalFilename", "systemprompt-cowork.exe");
        res.set("InternalName", "systemprompt-cowork");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=winresource compile failed: {e}");
        }
        println!("cargo:rerun-if-changed=assets/app-icon.ico");
        println!("cargo:rerun-if-changed=build.rs");
    }
}
