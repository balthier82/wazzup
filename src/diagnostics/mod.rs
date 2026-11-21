mod log_file;
mod scrub;

use std::path::PathBuf;

use gtk::glib;

use crate::config::{APP_ID, APP_NAME, Profile};
pub use log_file::RotatingLog;
use log_file::previous_path;
pub use scrub::Scrubber;

const REPORTED_LOG_LINES: usize = 120;

pub fn log_path() -> PathBuf {
    glib::user_state_dir().join(APP_ID).join(format!(
        "{}.log",
        Profile::from_environment().engine_folder()
    ))
}

pub fn environment() -> String {
    let read = |name: &str| std::env::var(name).unwrap_or_else(|_| "unknown".to_owned());
    let packaging = if std::env::var_os("APPIMAGE").is_some() {
        "AppImage"
    } else if std::env::var_os("FLATPAK_ID").is_some() {
        "Flatpak"
    } else {
        "native"
    };
    [
        format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")),
        format!("Engine: Chromium {}", cef::sys::CHROME_VERSION_MAJOR),
        format!(
            "GTK {}.{}.{}, libadwaita {}.{}.{}",
            gtk::major_version(),
            gtk::minor_version(),
            gtk::micro_version(),
            adw::major_version(),
            adw::minor_version(),
            adw::micro_version()
        ),
        format!("System: {}", operating_system()),
        format!(
            "Architecture: {}, packaging: {packaging}",
            std::env::consts::ARCH
        ),
        format!(
            "Session: {} on {}",
            read("XDG_SESSION_TYPE"),
            read("XDG_CURRENT_DESKTOP")
        ),
        format!("Language: {}", read("LANG")),
    ]
    .join("\n")
}

pub fn recent_log() -> String {
    let current = std::fs::read_to_string(log_path()).unwrap_or_default();
    let earlier = std::fs::read_to_string(previous_path(&log_path())).unwrap_or_default();
    let lines: Vec<&str> = earlier.lines().chain(current.lines()).collect();
    let start = lines.len().saturating_sub(REPORTED_LOG_LINES);
    lines[start..].join("\n")
}

fn operating_system() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|release| {
            release.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|name| name.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| "Linux".to_owned())
}
