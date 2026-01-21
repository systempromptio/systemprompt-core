#![allow(clippy::print_stdout)]

use crate::services::cli::theme::BrandColors;

pub fn render_startup_banner(subtitle: Option<&str>) {
    println!();
    println!(
        "{}{}{}{}{}",
        BrandColors::primary_bold("</"),
        BrandColors::white_bold("SYSTEMPROMPT"),
        BrandColors::primary_bold("."),
        BrandColors::white("io"),
        BrandColors::primary_bold(">")
    );
    if let Some(text) = subtitle {
        println!("{}", BrandColors::dim(text));
    }
    println!();
}

pub fn render_phase_header(name: &str) {
    println!(
        "\n{} {}",
        BrandColors::primary("▸"),
        BrandColors::white_bold(name)
    );
}

pub fn render_phase_item(icon: &str, message: &str, detail: Option<&str>) {
    match detail {
        Some(d) => println!(
            "  {} {} {}",
            icon,
            message,
            BrandColors::dim(format!("({})", d))
        ),
        None => println!("  {} {}", icon, message),
    }
}

pub fn render_phase_success(message: &str, detail: Option<&str>) {
    render_phase_item(&format!("{}", BrandColors::running("✓")), message, detail);
}

pub fn render_phase_info(message: &str, detail: Option<&str>) {
    render_phase_item(&format!("{}", BrandColors::highlight("ℹ")), message, detail);
}

pub fn render_phase_warning(message: &str, detail: Option<&str>) {
    render_phase_item(&format!("{}", BrandColors::starting("⚠")), message, detail);
}
