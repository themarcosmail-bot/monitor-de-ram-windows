#![cfg_attr(not(test), windows_subsystem = "windows")]

#[cfg(test)]
mod benchmark;
mod monitor;
mod settings;
mod ui;
mod windows;

fn main() {
    ui::run();
}
