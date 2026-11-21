use std::io;
use std::path::PathBuf;

use gtk::glib;

use crate::config::{APP_ID, APP_NAME};

pub trait Autostart {
    fn enable(&self) -> io::Result<()>;
    fn disable(&self) -> io::Result<()>;

    fn apply(&self, enabled: bool) -> io::Result<()> {
        if enabled {
            self.enable()
        } else {
            self.disable()
        }
    }
}

pub struct XdgAutostart {
    entry: PathBuf,
    command: String,
}

impl XdgAutostart {
    pub fn for_this_installation() -> Self {
        Self {
            entry: glib::user_config_dir()
                .join("autostart")
                .join(format!("{APP_ID}.desktop")),
            command: launcher_command(),
        }
    }

    fn entry_contents(&self) -> String {
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name={APP_NAME}\n\
             Icon={APP_ID}\n\
             Exec={} --background\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            self.command
        )
    }
}

impl Autostart for XdgAutostart {
    fn enable(&self) -> io::Result<()> {
        if let Some(folder) = self.entry.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(&self.entry, self.entry_contents())
    }

    fn disable(&self) -> io::Result<()> {
        match std::fs::remove_file(&self.entry) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

fn launcher_command() -> String {
    let program = ["WAZZUP_LAUNCHER", "APPIMAGE"]
        .into_iter()
        .find_map(std::env::var_os)
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("wazzup"));
    quote_for_exec(&program.to_string_lossy())
}

fn quote_for_exec(program: &str) -> String {
    let escaped: String = program
        .chars()
        .flat_map(|c| match c {
            '"' | '`' | '$' | '\\' => vec!['\\', c],
            '%' => vec!['%', '%'],
            other => vec![other],
        })
        .collect();
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::quote_for_exec;

    #[test]
    fn quotes_plain_paths() {
        assert_eq!(
            quote_for_exec("/opt/Wazzup.AppImage"),
            "\"/opt/Wazzup.AppImage\""
        );
    }

    #[test]
    fn escapes_reserved_characters() {
        assert_eq!(
            quote_for_exec("/home/me/My Apps/$x\"100%"),
            "\"/home/me/My Apps/\\$x\\\"100%%\""
        );
    }
}
