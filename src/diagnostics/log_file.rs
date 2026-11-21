use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

const ROTATE_AFTER_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct RotatingLog {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    path: PathBuf,
    file: Option<File>,
    written: u64,
}

impl RotatingLog {
    pub fn open(path: &Path) -> Self {
        let mut inner = Inner {
            path: path.to_path_buf(),
            file: None,
            written: 0,
        };
        inner.reopen();
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }
}

impl Inner {
    fn reopen(&mut self) {
        if let Some(folder) = self.path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok();
        self.written = self
            .file
            .as_ref()
            .and_then(|file| file.metadata().ok())
            .map_or(0, |metadata| metadata.len());
    }

    fn rotate(&mut self) {
        self.file = None;
        let _ = std::fs::rename(&self.path, previous_path(&self.path));
        self.reopen();
    }
}

pub fn previous_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

pub struct LogLine {
    inner: Arc<Mutex<Inner>>,
}

impl Write for LogLine {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("log lock poisoned"))?;
        if inner.written + buffer.len() as u64 > ROTATE_AFTER_BYTES {
            inner.rotate();
        }
        let Some(file) = inner.file.as_mut() else {
            return Ok(buffer.len());
        };
        file.write_all(buffer)?;
        inner.written += buffer.len() as u64;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("log lock poisoned"))?;
        inner.file.as_mut().map_or(Ok(()), Write::flush)
    }
}

impl<'a> MakeWriter<'a> for RotatingLog {
    type Writer = LogLine;

    fn make_writer(&'a self) -> Self::Writer {
        LogLine {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RotatingLog, previous_path};
    use std::io::Write;
    use tracing_subscriber::fmt::MakeWriter;

    #[test]
    fn rotates_large_logs() {
        let folder = std::env::temp_dir().join(format!("wazzup-log-{}", std::process::id()));
        let path = folder.join("wazzup.log");
        let log = RotatingLog::open(&path);
        let line = vec![b'x'; 512 * 1024];
        for _ in 0..5 {
            log.make_writer().write_all(&line).unwrap();
        }
        assert!(previous_path(&path).exists());
        assert!(std::fs::metadata(&path).unwrap().len() <= 2 * 1024 * 1024);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
