// Prevents an extra console window on Windows in release. DO NOT REMOVE.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Tauri entrypoint — defers to the lib so cargo tauri can call the same setup
// from the test/dev path.

fn main() {
    modula_desktop_lib::run();
}
