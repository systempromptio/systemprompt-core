use std::io::Write;

use crate::services::cli::theme::BrandColors;

fn stdout_writeln(args: std::fmt::Arguments<'_>) {
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{args}");
}

pub fn render_startup_banner(subtitle: Option<&str>) {
    stdout_writeln(format_args!(""));
    stdout_writeln(format_args!(
        "{}{}{}{}{}",
        BrandColors::primary_bold("</"),
        BrandColors::white_bold("SYSTEMPROMPT"),
        BrandColors::primary_bold("."),
        BrandColors::white("io"),
        BrandColors::primary_bold(">")
    ));
    if let Some(text) = subtitle {
        stdout_writeln(format_args!("{}", BrandColors::dim(text)));
    }
    stdout_writeln(format_args!(""));
}

pub fn render_phase_header(name: &str) {
    stdout_writeln(format_args!(
        "\n{} {}",
        BrandColors::primary("\u{25b8}"),
        BrandColors::white_bold(name)
    ));
}

pub fn render_phase_item(icon: &str, message: &str, detail: Option<&str>) {
    match detail {
        Some(d) => stdout_writeln(format_args!(
            "  {} {} {}",
            icon,
            message,
            BrandColors::dim(format!("({})", d))
        )),
        None => stdout_writeln(format_args!("  {} {}", icon, message)),
    }
}

pub fn render_phase_success(message: &str, detail: Option<&str>) {
    render_phase_item(
        &format!("{}", BrandColors::running("\u{2713}")),
        message,
        detail,
    );
}

pub fn render_phase_info(message: &str, detail: Option<&str>) {
    render_phase_item(
        &format!("{}", BrandColors::highlight("\u{2139}")),
        message,
        detail,
    );
}

pub fn render_phase_warning(message: &str, detail: Option<&str>) {
    render_phase_item(
        &format!("{}", BrandColors::starting("\u{26a0}")),
        message,
        detail,
    );
}
