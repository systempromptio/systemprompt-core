#![allow(clippy::print_stdout)]

use std::time::Duration;

use crate::services::cli::theme::{
    ActionType, BrandColors, EmphasisType, IconType, ItemStatus, MessageLevel, ModuleType,
    ServiceStatus, Theme,
};

pub trait Display {
    fn display(&self);
}

pub trait DetailedDisplay {
    fn display_summary(&self);
    fn display_details(&self);
}

#[derive(Debug, Copy, Clone)]
pub struct DisplayUtils;

impl DisplayUtils {
    pub fn message(level: MessageLevel, text: &str) {
        println!("{} {}", Theme::icon(level), Theme::color(text, level));
    }

    pub fn section_header(title: &str) {
        println!("\n{}", Theme::color(title, EmphasisType::Underlined));
    }

    pub fn subsection_header(title: &str) {
        println!("\n  {}", Theme::color(title, EmphasisType::Bold));
    }

    pub fn item(icon_type: impl Into<IconType>, name: &str, detail: Option<&str>) {
        match detail {
            Some(detail) => println!(
                "   {} {} {}",
                Theme::icon(icon_type),
                Theme::color(name, EmphasisType::Bold),
                Theme::color(detail, EmphasisType::Dim)
            ),
            None => println!(
                "   {} {}",
                Theme::icon(icon_type),
                Theme::color(name, EmphasisType::Bold)
            ),
        }
    }

    pub fn relationship(icon_type: impl Into<IconType>, from: &str, to: &str, status: ItemStatus) {
        println!(
            "   {} {} {} {} {}",
            Theme::icon(icon_type),
            Theme::color(from, EmphasisType::Highlight),
            Theme::icon(ActionType::Arrow),
            Theme::color(to, status),
            Theme::color(&format!("({})", status_text(status)), EmphasisType::Dim)
        );
    }

    pub fn module_status(module_name: &str, message: &str) {
        let module_label = format!("Module: {module_name}");
        println!(
            "{} {} {}",
            Theme::icon(ModuleType::Module),
            Theme::color(&module_label, EmphasisType::Highlight),
            Theme::color(message, EmphasisType::Dim)
        );
    }

    pub fn count_message(level: MessageLevel, count: usize, item_type: &str) {
        let count_label = format!("{} {item_type}", count_text(count, item_type));
        let count_str = count.to_string();
        println!(
            "   {} {}: {}",
            Theme::icon(level),
            count_label,
            Theme::color(&count_str, level)
        );
    }
}

#[derive(Debug)]
pub struct StatusDisplay {
    pub status: ItemStatus,
    pub name: String,
    pub detail: Option<String>,
}

impl StatusDisplay {
    pub fn new(status: ItemStatus, name: impl Into<String>) -> Self {
        Self {
            status,
            name: name.into(),
            detail: None,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl Display for StatusDisplay {
    fn display(&self) {
        DisplayUtils::item(self.status, &self.name, self.detail.as_deref());
    }
}

#[derive(Debug)]
pub struct ModuleItemDisplay {
    pub module_type: ModuleType,
    pub file: String,
    pub target: String,
    pub status: ItemStatus,
}

impl ModuleItemDisplay {
    pub fn new(
        module_type: ModuleType,
        file: impl Into<String>,
        target: impl Into<String>,
        status: ItemStatus,
    ) -> Self {
        Self {
            module_type,
            file: file.into(),
            target: target.into(),
            status,
        }
    }
}

impl Display for ModuleItemDisplay {
    fn display(&self) {
        DisplayUtils::relationship(self.module_type, &self.file, &self.target, self.status);
    }
}

#[derive(Debug)]
pub struct CollectionDisplay<T: Display> {
    pub title: String,
    pub items: Vec<T>,
    pub show_count: bool,
}

impl<T: Display> CollectionDisplay<T> {
    pub fn new(title: impl Into<String>, items: Vec<T>) -> Self {
        Self {
            title: title.into(),
            items,
            show_count: true,
        }
    }

    #[must_use]
    pub const fn without_count(mut self) -> Self {
        self.show_count = false;
        self
    }
}

impl<T: Display> Display for CollectionDisplay<T> {
    fn display(&self) {
        if self.show_count && !self.items.is_empty() {
            println!(
                "\n{} {}:",
                Theme::color(&self.title, EmphasisType::Bold),
                Theme::color(&format!("({})", self.items.len()), EmphasisType::Dim)
            );
        } else if !self.items.is_empty() {
            println!("\n{}:", Theme::color(&self.title, EmphasisType::Bold));
        }

        for item in &self.items {
            item.display();
        }
    }
}

const fn status_text(status: ItemStatus) -> &'static str {
    match status {
        ItemStatus::Missing => "missing",
        ItemStatus::Applied => "applied",
        ItemStatus::Failed => "failed",
        ItemStatus::Valid => "valid",
        ItemStatus::Disabled => "disabled",
        ItemStatus::Pending => "pending",
    }
}

fn count_text(count: usize, item_type: &str) -> &'static str {
    if count == 1 {
        match item_type {
            "schemas" => "Missing schema",
            "seeds" => "Missing seed",
            "modules" => "New module",
            _ => "Missing item",
        }
    } else {
        match item_type {
            "schemas" => "Missing schemas",
            "seeds" => "Missing seeds",
            "modules" => "New modules",
            _ => "Missing items",
        }
    }
}

fn truncate_to_width(s: &str, width: usize) -> String {
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
