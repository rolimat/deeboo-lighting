#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(e) = deeboo_lighting_lib::run() {
        eprintln!("Deeboo Lighting failed to start: {e}");
        std::process::exit(1);
    }
}
