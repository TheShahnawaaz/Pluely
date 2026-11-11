#[cfg(any(target_os = "macos", target_os = "windows"))]
mod capture;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod commands;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod desktop;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod llm;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod models;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod service;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn run() {
    desktop::run()
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn run() {
    eprintln!("Pluely desktop supports macOS and Windows. Core tests and the explicit browser UI preview are available on this host.")
}
