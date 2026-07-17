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
            .map(|s| s.name.len())
            .max()
            .unwrap_or(4)
            .max(4);
        let service_type = services
            .iter()
            .map(|s| s.service_type.len())
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

    fn rule(&self, left: &str, middle: &str, right: &str) {
        stdout_writeln(format_args!(
            "{left}{}{middle}{}{middle}{}{middle}{}{right}",
            "\u{2500}".repeat(self.name + 2),
            "\u{2500}".repeat(self.service_type + 2),
            "\u{2500}".repeat(self.port + 2),
            "\u{2500}".repeat(self.status + 2)
        ));
    }
}

pub fn render_service_table(title: &str, services: &[ServiceTableEntry]) {
    if services.is_empty() {
        return;
    }

    let cols = ServiceColumns::measure(services);
    let total_width = cols.name + cols.service_type + cols.port + cols.status + 13;

    stdout_writeln(format_args!(""));
    stdout_writeln(format_args!(
        "\u{250c}{}\u{2510}",
        "\u{2500}".repeat(total_width)
    ));
    stdout_writeln(format_args!(
        "\u{2502} {:<width$} \u{2502}",
        BrandColors::white_bold(title),
        width = total_width - 3
    ));

    cols.rule("\u{251c}", "\u{252c}", "\u{2524}");
    render_service_header(&cols);
    cols.rule("\u{251c}", "\u{253c}", "\u{2524}");

    for service in services {
        render_service_row(service, &cols);
    }

    cols.rule("\u{2514}", "\u{2534}", "\u{2518}");
}

fn render_service_header(cols: &ServiceColumns) {
    let name_width = cols.name;
    let type_width = cols.service_type;
    let port_width = cols.port;
    let status_width = cols.status;
    stdout_writeln(format_args!(
        "\u{2502} {:<name_width$} \u{2502} {:<type_width$} \u{2502} {:<port_width$} \u{2502} \
         {:<status_width$} \u{2502}",
        BrandColors::dim("Name"),
        BrandColors::dim("Type"),
        BrandColors::dim("Port"),
        BrandColors::dim("Status"),
    ));
}

fn render_service_row(service: &ServiceTableEntry, cols: &ServiceColumns) {
    let name_width = cols.name;
    let type_width = cols.service_type;
    let port_width = cols.port;
    let status_width = cols.status;

    let port_str = service
        .port
        .map_or_else(|| "-".to_owned(), |p| p.to_string());

    let status_display = format!("{} {}", service.status.symbol(), service.status.text());
    let colored_status = match service.status {
        ServiceStatus::Running => format!("{}", BrandColors::running(&status_display)),
        ServiceStatus::Starting => format!("{}", BrandColors::starting(&status_display)),
        ServiceStatus::Stopped | ServiceStatus::Failed => {
            format!("{}", BrandColors::stopped(&status_display))
        },
        ServiceStatus::Unknown => format!("{}", BrandColors::dim(&status_display)),
    };

    stdout_writeln(format_args!(
        "\u{2502} {:<name_width$} \u{2502} {:<type_width$} \u{2502} {:>port_width$} \u{2502} \
         {:<status_width$} \u{2502}",
        service.name, service.service_type, port_str, colored_status,
    ));
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
