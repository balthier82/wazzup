mod application;
mod browser;
mod call_window;
mod config;
mod engine;
mod navigation;
mod unread;
mod window;

use std::process::ExitCode;

use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    let engine = match engine::Engine::boot() {
        engine::Boot::ChildProcessFinished(code) => return ExitCode::from(code),
        engine::Boot::Failed(reason) => {
            eprintln!("wazzup: {reason}");
            return ExitCode::FAILURE;
        }
        engine::Boot::Ready(engine) => engine,
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("WAZZUP_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let status = application::WazzupApplication::new().run();
    engine.shutdown();
    if status == gtk::glib::ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
