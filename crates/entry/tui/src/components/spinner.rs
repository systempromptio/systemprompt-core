use ratatui::style::Color;
use std::sync::OnceLock;
use std::time::Instant;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

const FRAME_DURATION_MS: u128 = 80;

static START_TIME: OnceLock<Instant> = OnceLock::new();

const PROCESSING_COLORS: &[Color] = &[
    Color::Rgb(255, 165, 0),
    Color::Rgb(255, 190, 60),
    Color::White,
    Color::Rgb(255, 200, 120),
    Color::Rgb(255, 140, 0),
    Color::Rgb(255, 180, 80),
    Color::White,
    Color::Rgb(255, 160, 40),
    Color::Rgb(255, 200, 100),
    Color::Rgb(255, 150, 20),
];

const fn tick_index(ticks: u128, len: usize) -> usize {
    (ticks % len as u128) as usize
}

pub fn get_spinner_frame() -> char {
    let start = START_TIME.get_or_init(Instant::now);
    let ticks = start.elapsed().as_millis() / FRAME_DURATION_MS;
    SPINNER_FRAMES[tick_index(ticks, SPINNER_FRAMES.len())]
}

pub fn get_processing_spinner() -> (char, Color) {
    let start = START_TIME.get_or_init(Instant::now);
    let ticks = start.elapsed().as_millis() / FRAME_DURATION_MS;
    let frame_index = tick_index(ticks, SPINNER_FRAMES.len());
    let color_index = tick_index(ticks, PROCESSING_COLORS.len());
    (SPINNER_FRAMES[frame_index], PROCESSING_COLORS[color_index])
}

const DOT_DURATION_MS: u128 = 400;

pub fn get_animated_dots() -> &'static str {
    const DOTS: &[&str] = &["", ".", "..", "..."];
    let start = START_TIME.get_or_init(Instant::now);
    let ticks = start.elapsed().as_millis() / DOT_DURATION_MS;
    DOTS[tick_index(ticks, DOTS.len())]
}
