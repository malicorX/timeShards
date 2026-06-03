#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    timeshards_server_lib::run();
}
