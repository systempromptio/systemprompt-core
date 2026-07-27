//! Box-drawing table renderers for CLI output.
//!
//! [`render_table`] draws an arbitrary header/row grid;
//! [`render_service_table`] renders the service-status table from
//! [`ServiceTableEntry`] values; and [`render_startup_complete`] prints the
//! post-boot summary. Output goes to stdout via this sanctioned display sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;
use std::time::Duration;

use crate::services::cli::theme::{BrandColors, ServiceStatus};

fn stdout_write(args: std::fmt::Arguments<'_>) {
    let mut out = std::io::stdout();
    write!(out, "{args}").ok();
}

fn stdout_writeln(args: std::fmt::Arguments<'_>) {
    let mut out = std::io::stdout();
    writeln!(out, "{args}").ok();
}

#[derive(Debug, Clone)]
pub struct ServiceTableEntry {
    pub name: String,
    pub service_type: String,
    pub port: Option<u16>,
    pub status: ServiceStatus,
}

impl ServiceTableEntry {
    pub fn new(
        name: impl Into<String>,
        service_type: impl Into<String>,
        port: Option<u16>,
        status: ServiceStatus,
    ) -> Self {
        Self {
            name: name.into(),
            service_type: service_type.into(),
            port,
            status,
        }
    }
}

pub fn truncate_to_width(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_owned();
    }
    let truncate_to = width.saturating_sub(3);
    let truncated: String = s.chars().take(truncate_to).collect();
    format!("{truncated}...")
}

fn calculate_column_widths(headers: &[&str], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    widths
}

fn render_table_border(widths: &[usize], left: &str, middle: &str, right: &str) {
    stdout_write(format_args!("{left}"));
    for (i, &width) in widths.iter().enumerate() {
        stdout_write(format_args!("{}", "\u{2500}".repeat(width + 2)));
        if i < widths.len() - 1 {
            stdout_write(format_args!("{middle}"));
        }
    }
    stdout_writeln(format_args!("{right}"));
}

fn render_table_row(cells: &[&str], widths: &[usize]) {
    stdout_write(format_args!("\u{2502}"));
    for (i, (&cell, &width)) in cells.iter().zip(widths.iter()).enumerate() {
        let truncated = truncate_to_width(cell, width);
        stdout_write(format_args!(" {truncated:<width$} "));
        if i < widths.len() - 1 {
            stdout_write(format_args!("\u{2502}"));
        }
    }
    stdout_writeln(format_args!("\u{2502}"));
}

pub fn render_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }

    let widths = calculate_column_widths(headers, rows);

    render_table_border(&widths, "\u{250c}", "\u{252c}", "\u{2510}");
    render_table_row(headers, &widths);
    render_table_border(&widths, "\u{251c}", "\u{253c}", "\u{2524}");

    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        render_table_row(&cells, &widths);
    }

    render_table_border(&widths, "\u{2514}", "\u{2534}", "\u{2518}");
}

struct ServiceColumns {
    name: usize,
    service_type: usize,
    port: usize,
    status: usize,
}

impl ServiceColumns {
    fn measure(services: &[ServiceTableEntry]) -> Self {
        let name = services
            .iter()
            .map(|s| s.name.chars().count())
            .max()
            .unwrap_or(4)
            .max(4);
        let service_type = services
            .iter()
            .map(|s| s.service_type.chars().count())
            .max()
            .unwrap_or(4)
            .max(4);
        Self {
            name,
            service_type,
            port: 5,
            status: 10,
        }
    }

    /// Display width of a row's interior, i.e. everything strictly between the
    /// two outer box-drawing glyphs. Every line of the table is framed against
    /// this one number so the borders, title, header, and rows cannot drift.
    const fn interior_width(&self) -> usize {
        (self.name + 2) + (self.service_type + 2) + (self.port + 2) + (self.status + 2) + 3
    }

    fn rule(
        &self,
        out: &mut impl Write,
        left: &str,
        middle: &str,
        right: &str,
    ) -> std::io::Result<()> {
        writeln!(
            out,
            "{left}{}{middle}{}{middle}{}{middle}{}{right}",
            "\u{2500}".repeat(self.name + 2),
            "\u{2500}".repeat(self.service_type + 2),
            "\u{2500}".repeat(self.port + 2),
            "\u{2500}".repeat(self.status + 2)
        )
    }
}

pub fn render_service_table(title: &str, services: &[ServiceTableEntry]) {
    render_service_table_into(&mut std::io::stdout(), title, services).ok();
}

/// Renders the service-status table into `out`.
///
/// Split from [`render_service_table`] so the frame geometry is assertable: the
/// stdout entry point above discards the result, which is why an off-by-two in
/// the top border went unnoticed.
pub fn render_service_table_into(
    out: &mut impl Write,
    title: &str,
    services: &[ServiceTableEntry],
) -> std::io::Result<()> {
    if services.is_empty() {
        return Ok(());
    }

    let cols = ServiceColumns::measure(services);
    let interior = cols.interior_width();

    writeln!(out)?;
    writeln!(out, "\u{250c}{}\u{2510}", "\u{2500}".repeat(interior))?;
    writeln!(
        out,
        "\u{2502} {:<width$} \u{2502}",
        BrandColors::white_bold(title),
        width = interior - 2
    )?;

    cols.rule(out, "\u{251c}", "\u{252c}", "\u{2524}")?;
    render_service_header(out, &cols)?;
    cols.rule(out, "\u{251c}", "\u{253c}", "\u{2524}")?;

    for service in services {
        render_service_row(out, service, &cols)?;
    }

    cols.rule(out, "\u{2514}", "\u{2534}", "\u{2518}")
}

fn render_service_header(out: &mut impl Write, cols: &ServiceColumns) -> std::io::Result<()> {
    let name_width = cols.name;
    let type_width = cols.service_type;
    let port_width = cols.port;
    let status_width = cols.status;
    writeln!(
        out,
        "\u{2502} {:<name_width$} \u{2502} {:<type_width$} \u{2502} {:>port_width$} \u{2502} \
         {:<status_width$} \u{2502}",
        BrandColors::dim("Name"),
        BrandColors::dim("Type"),
        BrandColors::dim("Port"),
        BrandColors::dim("Status"),
    )
}

fn render_service_row(
    out: &mut impl Write,
    service: &ServiceTableEntry,
    cols: &ServiceColumns,
) -> std::io::Result<()> {
    let name_width = cols.name;
    let type_width = cols.service_type;
    let port_width = cols.port;

    let port_str = service
        .port
        .map_or_else(|| "-".to_owned(), |p| p.to_string());

    let status_display = format!("{} {}", service.status.symbol(), service.status.text());
    // Why: pad before styling. A styled `String` carries ANSI escapes, and
    // `str`'s formatter counts those bytes as content, so padding a
    // pre-rendered status collapses the column.
    let padded_status = format!("{status_display:<width$}", width = cols.status);
    let colored_status = match service.status {
        ServiceStatus::Running => BrandColors::running(padded_status),
        ServiceStatus::Starting => BrandColors::starting(padded_status),
        ServiceStatus::Stopped | ServiceStatus::Failed => BrandColors::stopped(padded_status),
        ServiceStatus::Unknown => BrandColors::dim(padded_status),
    };

    writeln!(
        out,
        "\u{2502} {:<name_width$} \u{2502} {:<type_width$} \u{2502} {:>port_width$} \u{2502} \
         {colored_status} \u{2502}",
        truncate_to_width(&service.name, name_width),
        truncate_to_width(&service.service_type, type_width),
        port_str,
    )
}

pub fn render_startup_complete(duration: Duration, api_url: &str) {
    let secs = duration.as_secs_f64();
    stdout_writeln(format_args!(""));
    stdout_writeln(format_args!(
        "{} {} {}",
        BrandColors::running("\u{2713}"),
        BrandColors::white_bold("All services started successfully"),
        BrandColors::dim(format!("({:.1}s)", secs))
    ));
    stdout_writeln(format_args!(
        "  {} {}",
        BrandColors::dim("API:"),
        BrandColors::highlight(api_url)
    ));
    stdout_writeln(format_args!(""));
}
