use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::DownloadState;

const KEPT_RECORDS: usize = 50;
const RISKY_EXTENSIONS: &[&str] = &[
    "appimage", "bash", "bat", "bin", "cmd", "com", "desktop", "deb", "exe", "jar", "js", "msi",
    "pl", "ps1", "py", "rb", "rpm", "run", "scr", "sh", "vbs", "zsh",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DownloadRecord {
    pub path: PathBuf,
    pub received: u64,
    pub total: Option<u64>,
    pub state: DownloadState,
}

impl DownloadRecord {
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_default(|name| name.to_string_lossy().into_owned())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub records: Vec<DownloadRecord>,
}

impl History {
    pub fn from_toml(source: &str) -> Self {
        let mut history: Self = toml::from_str(source).unwrap_or_default();
        for record in &mut history.records {
            if record.state == DownloadState::Running {
                record.state = DownloadState::Failed;
            }
        }
        history
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    pub fn remember(&mut self, record: DownloadRecord) {
        self.records.retain(|known| known.path != record.path);
        self.records.insert(0, record);
        self.records.truncate(KEPT_RECORDS);
    }

    pub fn forget_finished(&mut self) {
        self.records
            .retain(|record| record.state == DownloadState::Running);
    }
}

pub fn is_risky(path: &Path) -> bool {
    let by_extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|extension| RISKY_EXTENSIONS.contains(&extension.as_str()));
    by_extension || is_executable(path)
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

pub fn safe_file_name(suggested: &str) -> String {
    let cleaned: String = suggested
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "download".to_owned()
    } else {
        trimmed.chars().take(200).collect()
    }
}

pub fn unused_path(folder: &Path, suggested: &str) -> PathBuf {
    let name = safe_file_name(suggested);
    let candidate = folder.join(&name);
    if !candidate.exists() {
        return candidate;
    }
    let as_path = Path::new(&name);
    let stem = as_path
        .file_stem()
        .map_or_else(|| name.clone(), |stem| stem.to_string_lossy().into_owned());
    let extension = as_path
        .extension()
        .map_or_default(|extension| format!(".{}", extension.to_string_lossy()));
    (1..=9999)
        .map(|index| folder.join(format!("{stem} ({index}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(candidate)
}

pub fn readable_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024 * 1024 && unit + 1 < UNITS.len() {
        value /= 1024;
        unit += 1;
    }
    let tenths = value * 10 / 1024;
    format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::{DownloadRecord, History, is_risky, readable_size, safe_file_name, unused_path};
    use crate::engine::DownloadState;
    use std::path::{Path, PathBuf};

    fn record(name: &str, state: DownloadState) -> DownloadRecord {
        DownloadRecord {
            path: PathBuf::from(format!("/tmp/{name}")),
            received: 1,
            total: Some(1),
            state,
        }
    }

    #[test]
    fn cleans_file_names() {
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name("..."), "download");
        assert_eq!(safe_file_name(" photo.jpg "), "photo.jpg");
        assert_eq!(safe_file_name(".bashrc"), "bashrc");
    }

    #[test]
    fn avoids_overwriting() {
        let folder = std::env::temp_dir().join(format!("wazzup-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("note.txt"), "x").unwrap();
        std::fs::write(folder.join("note (1).txt"), "x").unwrap();
        assert_eq!(
            unused_path(&folder, "note.txt"),
            folder.join("note (2).txt")
        );
        assert_eq!(unused_path(&folder, "other.txt"), folder.join("other.txt"));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn flags_dangerous_files() {
        assert!(is_risky(Path::new("/tmp/install.SH")));
        assert!(is_risky(Path::new("/tmp/app.desktop")));
        assert!(!is_risky(Path::new("/tmp/does-not-exist/photo.jpg")));
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(readable_size(512), "512 B");
        assert_eq!(readable_size(1536), "1.5 KB");
        assert_eq!(readable_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn keeps_recent_records_once() {
        let mut history = History::default();
        history.remember(record("a", DownloadState::Running));
        history.remember(record("b", DownloadState::Finished));
        history.remember(record("a", DownloadState::Finished));
        assert_eq!(history.records.len(), 2);
        assert_eq!(history.records[0].file_name(), "a");
    }

    #[test]
    fn marks_interrupted_downloads_as_failed() {
        let mut history = History::default();
        history.remember(record("a", DownloadState::Running));
        let reloaded = History::from_toml(&history.to_toml());
        assert_eq!(reloaded.records[0].state, DownloadState::Failed);
    }
}
