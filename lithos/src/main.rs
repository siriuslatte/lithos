use dotenv::dotenv;
use log::info;

mod cli;
mod commands;
mod preview;
mod tui;
mod ui;

#[cfg(windows)]
fn enable_utf8_console() {
    extern "system" {
        fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
        fn SetConsoleCP(wCodePageID: u32) -> i32;
    }
    const CP_UTF8: u32 = 65001;
    unsafe {
        SetConsoleOutputCP(CP_UTF8);
        SetConsoleCP(CP_UTF8);
    }
}

#[cfg(not(windows))]
fn enable_utf8_console() {}

#[tokio::main]
async fn main() {
    enable_utf8_console();

    let dotenv_path = dotenv().ok();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    if let Some(path) = dotenv_path {
        info!("Loaded variables from dotenv file: {}", path.display());
    }

    let exit_code = cli::run().await;
    std::process::exit(exit_code);
}
