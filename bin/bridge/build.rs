#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "cargo build-script protocol uses stdout for `cargo:` directives and stderr for \
              diagnostics"
)]
#![allow(
    clippy::expect_used,
    reason = "panicking is the conventional build-script failure mode; a broken web overlay must fail the build"
)]

fn emit_vergen() -> Result<(), Box<dyn std::error::Error>> {
    let build = vergen_gitcl::Build::builder().build_timestamp(true).build();
    let gitcl = vergen_gitcl::Gitcl::builder()
        .sha(true)
        .commit_date(true)
        .branch(true)
        .build();
    vergen_gitcl::Emitter::default()
        .fail_on_error()
        .add_instructions(&build)?
        .add_instructions(&gitcl)?
        .emit()?;
    Ok(())
}

fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
    for entry in std::fs::read_dir(src).expect("read_dir web asset source") {
        let entry = entry.expect("read web asset dir entry");
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().expect("web asset file type").is_dir() {
            std::fs::create_dir_all(&to).expect("create staged asset subdir");
            copy_tree(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("copy web asset into OUT_DIR");
        }
    }
}

fn stage_web_assets() {
    // Stage the GUI web tree into OUT_DIR so `assets.rs` can `include_str!` from
    // there, and apply an optional brand overlay on top. This is what lets a
    // white-label repo replace any css/js/html/i18n file without editing core:
    // its build sets SYSTEMPROMPT_BRIDGE_WEB_OVERLAY (see astound bridge
    // .cargo/config.toml) and those files win over the staged core copies.
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    let staged = std::path::Path::new(&out_dir).join("web");
    std::fs::create_dir_all(&staged).expect("create staged web dir");
    copy_tree(std::path::Path::new("web"), &staged);
    println!("cargo:rerun-if-changed=web");

    println!("cargo:rerun-if-env-changed=SYSTEMPROMPT_BRIDGE_WEB_OVERLAY");
    if let Ok(overlay) = std::env::var("SYSTEMPROMPT_BRIDGE_WEB_OVERLAY") {
        let overlay = std::path::Path::new(&overlay);
        if overlay.is_dir() {
            copy_tree(overlay, &staged);
            println!("cargo:rerun-if-changed={}", overlay.display());
        } else {
            println!(
                "cargo:warning=SYSTEMPROMPT_BRIDGE_WEB_OVERLAY={} is not a directory; ignoring",
                overlay.display()
            );
        }
    }

    write_web_manifest(&staged, std::path::Path::new(&out_dir));
}

fn is_served(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("css" | "js" | "ftl" | "html")
    )
}

fn collect_served(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read staged web dir") {
        let entry = entry.expect("read staged web dir entry");
        let path = entry.path();
        if entry.file_type().expect("staged web file type").is_dir() {
            collect_served(root, &path, out);
        } else if is_served(&path) {
            let rel = path
                .strip_prefix(root)
                .expect("staged file under staged root")
                .to_str()
                .expect("staged asset path is utf-8")
                .to_owned();
            out.push(rel);
        }
    }
}

fn write_web_manifest(staged: &std::path::Path, out_dir: &std::path::Path) {
    // Generate the asset routing table from the staged tree (core + overlay)
    // instead of hand-maintaining a list in assets.rs. Every servable file that
    // exists after staging is embedded and routable — a newly added module can
    // no longer 404 at runtime because someone forgot to register it.
    let mut rels = Vec::new();
    collect_served(staged, staged, &mut rels);
    rels.sort();

    let mut src = String::from("static WEB_TEXT_ASSETS: &[(&str, &str)] = &[\n");
    for rel in &rels {
        if rel == "index.html" {
            // Served via render_index(), which fills the template placeholders.
            continue;
        }
        let abs = staged.join(rel);
        src.push_str(&format!(
            "    ({rel:?}, include_str!({:?})),\n",
            abs.display()
        ));
    }
    src.push_str("];\n");
    std::fs::write(out_dir.join("web_manifest.rs"), src).expect("write web_manifest.rs");
}

fn main() {
    stage_web_assets();

    if let Err(e) = emit_vergen() {
        eprintln!("cargo:warning=vergen failed ({e}); falling back to placeholders");
        println!("cargo:rustc-env=VERGEN_GIT_SHA=unknown");
        println!("cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=unknown");
        println!("cargo:rustc-env=VERGEN_BUILD_TIMESTAMP=unknown");
        println!("cargo:rustc-env=VERGEN_GIT_BRANCH=unknown");
    }

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
