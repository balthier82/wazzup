use std::path::{Path, PathBuf};

use gtk::glib;

use crate::config::{APP_ID, Profile};

const STRIKES_BEFORE_FALLBACK: u32 = 2;

pub struct StartupGuard {
    marker: PathBuf,
    strikes: u32,
}

impl StartupGuard {
    pub fn arm() -> Self {
        let marker = glib::user_cache_dir().join(APP_ID).join(format!(
            "{}-starting",
            Profile::from_environment().engine_folder()
        ));
        Self::arm_at(&marker)
    }

    fn arm_at(marker: &Path) -> Self {
        let strikes = std::fs::read_to_string(marker)
            .ok()
            .map_or(0, |content| content.trim().parse::<u32>().unwrap_or(0) + 1);
        let guard = Self {
            marker: marker.to_path_buf(),
            strikes,
        };
        guard.write();
        guard
    }

    pub fn needs_software_rendering(&self) -> bool {
        self.strikes >= STRIKES_BEFORE_FALLBACK
    }

    pub fn settle(&self) {
        if let Err(error) = std::fs::remove_file(&self.marker)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "cannot clear startup marker");
        }
    }

    fn write(&self) {
        let written = self
            .marker
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&self.marker, self.strikes.to_string()));
        if let Err(error) = written {
            tracing::warn!(%error, "cannot write startup marker");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StartupGuard;

    fn marker(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("wazzup-guard-{name}-{}", std::process::id()))
    }

    #[test]
    fn clean_starts_keep_the_gpu() {
        let path = marker("clean");
        let first = StartupGuard::arm_at(&path);
        assert!(!first.needs_software_rendering());
        first.settle();
        let second = StartupGuard::arm_at(&path);
        assert!(!second.needs_software_rendering());
        second.settle();
    }

    #[test]
    fn repeated_failed_starts_fall_back() {
        let path = marker("failing");
        let _ = StartupGuard::arm_at(&path);
        let second = StartupGuard::arm_at(&path);
        assert!(!second.needs_software_rendering());
        let third = StartupGuard::arm_at(&path);
        assert!(third.needs_software_rendering());
        third.settle();
    }
}
