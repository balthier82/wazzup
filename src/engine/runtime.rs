use std::path::{Path, PathBuf};

use cef::args::Args;
use cef::{CefString, LogSeverity, Settings, api_hash, execute_process, initialize, shutdown, sys};
use gtk::glib;

use super::options::EngineOptions;
use super::process::WazzupApp;
use super::pump::MessagePump;
use crate::config::{APP_ID, Profile};

pub fn run_child_process() -> Option<u8> {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = Args::new();
    let mut app = WazzupApp::new(Vec::new());
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    (code >= 0).then(|| u8::try_from(code).unwrap_or(u8::MAX))
}

pub struct Engine {
    layout: RuntimeLayout,
}

impl Engine {
    pub fn start(options: &EngineOptions) -> Result<Self, String> {
        let args = Args::new();
        let mut app = WazzupApp::new(options.switches());
        let layout = RuntimeLayout::discover();
        layout
            .prepare()
            .map_err(|error| format!("cannot create profile folders: {error}"))?;
        if let Err(error) = options.write_profile_preferences(&layout.root_dir) {
            tracing::warn!(%error, "cannot update engine preferences");
        }
        let settings = layout.settings();
        let started = initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        );
        if started != 1 {
            return Err(format!(
                "the Chromium engine did not start (resources expected in {})",
                layout.engine_dir.display()
            ));
        }
        MessagePump::start_fallback();
        Ok(Self { layout })
    }

    pub fn shutdown(self) {
        tracing::info!(profile = %self.layout.profile_dir.display(), "engine shutting down");
        shutdown();
    }
}

struct RuntimeLayout {
    engine_dir: PathBuf,
    root_dir: PathBuf,
    profile_dir: PathBuf,
    language: String,
}

impl RuntimeLayout {
    fn discover() -> Self {
        let root_dir = glib::user_data_dir()
            .join(APP_ID)
            .join(Profile::from_environment().engine_folder());
        Self {
            engine_dir: engine_dir(),
            profile_dir: root_dir.join("default"),
            root_dir,
            language: preferred_language(),
        }
    }

    fn prepare(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.profile_dir)
    }

    fn settings(&self) -> Settings {
        Settings {
            no_sandbox: 1,
            windowless_rendering_enabled: 1,
            external_message_pump: 1,
            persist_session_cookies: 1,
            root_cache_path: path_string(&self.root_dir),
            cache_path: path_string(&self.profile_dir),
            resources_dir_path: path_string(&self.engine_dir),
            locales_dir_path: path_string(&self.engine_dir.join("locales")),
            locale: CefString::from(self.language.as_str()),
            accept_language_list: CefString::from(accept_languages(&self.language).as_str()),
            log_severity: LogSeverity::WARNING,
            remote_debugging_port: inspector_port(),
            ..Default::default()
        }
    }
}

fn engine_dir() -> PathBuf {
    if let Some(configured) = std::env::var_os("WAZZUP_ENGINE_DIR") {
        return PathBuf::from(configured);
    }
    let program_folder = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    [program_folder.join("../lib"), program_folder.clone()]
        .into_iter()
        .find(|candidate| candidate.join("icudtl.dat").is_file())
        .unwrap_or(program_folder)
}

fn inspector_port() -> i32 {
    if !cfg!(debug_assertions) {
        return 0;
    }
    std::env::var("WAZZUP_INSPECTOR_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or_default()
}

fn path_string(path: &Path) -> CefString {
    CefString::from(path.to_string_lossy().as_ref())
}

fn preferred_language() -> String {
    glib::language_names()
        .iter()
        .map(|name| {
            name.split(['_', '.', '@'])
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .find(|code| code.len() == 2 && code.chars().all(|c| c.is_ascii_lowercase()))
        .unwrap_or_else(|| "en".to_owned())
}

fn accept_languages(language: &str) -> String {
    if language == "en" {
        "en-US,en".to_owned()
    } else {
        format!("{language},en-US,en")
    }
}

#[cfg(test)]
mod tests {
    use super::accept_languages;

    #[test]
    fn keeps_english_as_fallback() {
        assert_eq!(accept_languages("it"), "it,en-US,en");
        assert_eq!(accept_languages("en"), "en-US,en");
    }
}
