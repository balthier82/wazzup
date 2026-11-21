use std::path::PathBuf;

use cef::{DownloadItem, DownloadItemCallback, ImplDownloadItem, ImplDownloadItemCallback};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadState {
    Running,
    Finished,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadProgress {
    pub id: u32,
    pub path: PathBuf,
    pub received: u64,
    pub total: Option<u64>,
    pub state: DownloadState,
}

impl DownloadProgress {
    pub fn read(item: &DownloadItem) -> Self {
        let state = if item.is_complete() == 1 {
            DownloadState::Finished
        } else if item.is_canceled() == 1 {
            DownloadState::Cancelled
        } else if item.is_interrupted() == 1 {
            DownloadState::Failed
        } else {
            DownloadState::Running
        };
        Self {
            id: item.id(),
            path: PathBuf::from(cef::CefString::from(&item.full_path()).to_string()),
            received: u64::try_from(item.received_bytes()).unwrap_or_default(),
            total: u64::try_from(item.total_bytes())
                .ok()
                .filter(|total| *total > 0),
            state,
        }
    }
}

#[derive(Clone)]
pub struct DownloadControl(DownloadItemCallback);

impl DownloadControl {
    pub fn new(callback: DownloadItemCallback) -> Self {
        Self(callback)
    }

    pub fn cancel(&self) {
        self.0.cancel();
    }
}
