#![allow(clippy::print_stdout)]

use std::time::Duration;

use crate::services::cli::theme::{BrandColors, ServiceStatus};

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
        return s.to_string();
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
    print!("{left}");
    for (i, &width) in widths.iter().enumerate() {
        print!("{}", "─".repeat(width + 2));
        if i < widths.len() - 1 {
            print!("{middle}");
        }
    }
    println!("{right}");
}

fn render_table_row(cells: &[&str], widths: &[usize]) {
    print!("│");
    for (i, (&cell, &width)) in cells.iter().zip(widths.iter()).enumerate() {
        let truncated = truncate_to_width(cell, width);
        print!(" {truncated:<width$} ");
        if i < widths.len() - 1 {
            print!("│");
        }
    }
    println!("│");
}

pub fn render_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }

    let widths = calculate_column_widths(headers, rows);

    render_table_border(&widths, "┌", "┬", "┐");
    render_table_row(headers, &widths);
    render_table_border(&widths, "├", "┼", "┤");

    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        render_table_row(&cells, &widths);
    }

    render_table_border(&widths, "└", "┴", "┘");
}

pub fn render_service_table(title: &str, services: &[ServiceTableEntry]) {
    if services.is_empty() {
        return;
    }

    let name_width = services
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let type_width = services
        .iter()
        .map(|s| s.service_type.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let port_width = 5;
    let status_width = 10;

    let total_width = name_width + type_width + port_width + status_width + 13;

    println!();
    println!("┌{}┐", "─".repeat(total_width));
    println!(
        "│ {:<width$} │",
        BrandColors::white_bold(title),
        width = total_width - 3
    );

    println!(
        "├{}┬{}┬{}┬{}┤",
        "─".repeat(name_width + 2),
        "─".repeat(type_width + 2),
        "─".repeat(port_width + 2),
        "─".repeat(status_width + 2)
    );

    println!(
        "│ {:<name_width$} │ {:<type_width$} │ {:<port_width$} │ {:<status_width$} │",
        BrandColors::dim("Name"),
        BrandColors::dim("Type"),
        BrandColors::dim("Port"),
        BrandColors::dim("Status"),
    );

    println!(
        "├{}┼{}┼{}┼{}┤",
        "─".repeat(name_width + 2),
        "─".repeat(type_width + 2),
        "─".repeat(port_width + 2),
        "─".repeat(status_width + 2)
    );

    for service in services {
        let port_str = service
            .port
            .map_or_else(|| "-".to_string(), |p| p.to_string());

        let status_display = format!("{} {}", service.status.symbol(), service.status.text());
        let colored_status = match service.status {
            ServiceStatus::Running => format!("{}", BrandColors::running(&status_display)),
            ServiceStatus::Starting => format!("{}", BrandColors::starting(&status_display)),
            ServiceStatus::Stopped | ServiceStatus::Failed => {
                format!("{}", BrandColors::stopped(&status_display))
            },
            ServiceStatus::Unknown => format!("{}", BrandColors::dim(&status_display)),
        };

        println!(
            "│ {:<name_width$} │ {:<type_width$} │ {:>port_width$} │ {:<status_width$} │",
            service.name, service.service_type, port_str, colored_status,
        );
    }

    println!(
        "└{}┴{}┴{}┴{}┘",
        "─".repeat(name_width + 2),
        "─".repeat(type_width + 2),
        "─".repeat(port_width + 2),
        "─".repeat(status_width + 2)
    );
}

pub fn render_startup_complete(duration: Duration, api_url: &str) {
    let secs = duration.as_secs_f64();
    println!();
    println!(
        "{} {} {}",
        BrandColors::running("✓"),
        BrandColors::white_bold("All services started successfully"),
        BrandColors::dim(format!("({:.1}s)", secs))
    );
    println!(
        "  {} {}",
        BrandColors::dim("API:"),
        BrandColors::highlight(api_url)
    );
    println!();
}
