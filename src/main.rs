mod application;
mod browser;
mod call_window;
mod chat_link;
mod config;
mod desktop;
mod downloads;
mod engine;
mod i18n;
mod launch;
mod lock;
mod navigation;
mod preferences;
mod preferences_dialog;
mod proxy;
mod recovery;
mod spelling;
mod startup_guard;
mod tray;
mod unread;
mod window;

use std::process::ExitCode;

use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    if let Some(code) = engine::run_child_process() {
        return ExitCode::from(code);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("WAZZUP_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let application = application::WazzupApplication::new();
    match application.register() {
        Ok(true) => return exit_code(application.run()),
        Ok(false) => {}
        Err(error) => {
            eprintln!("wazzup: cannot reach the desktop session: {error}");
            return ExitCode::FAILURE;
        }
    }

    application.activate_language();
    let guard = std::rc::Rc::new(startup_guard::StartupGuard::arm());
    let options = application.engine_options(guard.needs_software_rendering());
    let engine = match engine::Engine::start(&options) {
        Ok(engine) => engine,
        Err(reason) => {
            eprintln!("wazzup: {reason}");
            return ExitCode::FAILURE;
        }
    };
    let settled = std::rc::Rc::clone(&guard);
    gtk::glib::timeout_add_seconds_local_once(30, move || settled.settle());
    let status = application.run();
    guard.settle();
    engine.shutdown();
    exit_code(status)
}

fn exit_code(status: gtk::glib::ExitCode) -> ExitCode {
    if status == gtk::glib::ExitCode::SUCCESS {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
