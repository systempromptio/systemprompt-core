#![allow(clippy::print_stdout, clippy::print_stderr)]

pub fn print_line(msg: &str) {
    println!("{msg}");
}

pub fn print_str(msg: &str) {
    print!("{msg}");
}

pub fn eprint_str(msg: &str) {
    eprint!("{msg}");
}
