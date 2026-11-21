use std::path::PathBuf;

use cef::{CefString, CefStringList, FileDialogCallback, FileDialogMode, ImplFileDialogCallback};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChoice {
    OpenOne,
    OpenMany,
    OpenFolder,
    Save,
}

impl FileChoice {
    pub fn from_mode(mode: FileDialogMode) -> Self {
        match mode {
            FileDialogMode::OPEN_MULTIPLE => Self::OpenMany,
            FileDialogMode::OPEN_FOLDER => Self::OpenFolder,
            FileDialogMode::SAVE => Self::Save,
            _ => Self::OpenOne,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRequest {
    pub choice: FileChoice,
    pub title: String,
    pub suggested: Option<PathBuf>,
    pub accepted: Vec<String>,
}

impl FileRequest {
    pub fn new(
        mode: FileDialogMode,
        title: Option<&CefString>,
        suggested: Option<&CefString>,
        filters: Option<&CefStringList>,
    ) -> Self {
        let text = |value: Option<&CefString>| value.map_or_default(ToString::to_string);
        let suggested = text(suggested);
        Self {
            choice: FileChoice::from_mode(mode),
            title: text(title),
            suggested: (!suggested.is_empty()).then(|| PathBuf::from(suggested)),
            accepted: filters.map_or_default(|filters| {
                filters
                    .clone()
                    .into_iter()
                    .flat_map(|filter| split_filter(&filter))
                    .collect()
            }),
        }
    }
}

fn split_filter(filter: &str) -> Vec<String> {
    let patterns = filter
        .split_once('|')
        .map_or(filter, |(_, patterns)| patterns);
    patterns
        .split([';', ','])
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(str::to_owned)
        .collect()
}

pub struct FileAnswer(FileDialogCallback);

impl FileAnswer {
    pub fn new(callback: FileDialogCallback) -> Self {
        Self(callback)
    }

    pub fn choose(self, paths: &[PathBuf]) {
        if paths.is_empty() {
            self.0.cancel();
            return;
        }
        let mut list = CefStringList::new();
        for path in paths {
            list.append(&path.to_string_lossy());
        }
        self.0.cont(Some(&mut list));
    }

    pub fn dismiss(self) {
        self.0.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::split_filter;

    #[test]
    fn reads_plain_and_described_filters() {
        assert_eq!(split_filter("image/*"), vec!["image/*"]);
        assert_eq!(split_filter("Images|.png;.jpg"), vec![".png", ".jpg"]);
        assert_eq!(split_filter(" .pdf , .txt "), vec![".pdf", ".txt"]);
    }
}
