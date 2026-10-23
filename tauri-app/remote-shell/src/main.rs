#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod desktop;

fn main() {
    #[cfg(windows)]
    desktop::run();
    #[cfg(not(windows))]
    eprintln!(
        "The remote desktop shell targets Windows. Use cargo run --example probe for the transport smoke test."
    );
}
