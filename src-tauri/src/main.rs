// Release builds must use the Windows GUI subsystem: a console subsystem opens a
// terminal next to the window, and closing it terminates the app without saving.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    columnia_lib::run();
}
