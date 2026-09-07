#![allow(clippy::all)]

#[cfg(test)]
mod black_box;

#[cfg(all(test, target_os = "linux"))]
mod update_flows;
