use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::config::{APP_ID, Profile};
use crate::proxy::ProxySettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CloseAction {
    #[default]
    KeepInBackground,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationDetail {
    #[default]
    SenderAndMessage,
    SenderOnly,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrayStyle {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Veil {
    #[default]
    Off,
    Light,
    Medium,
    Strong,
}

impl Veil {
    pub fn pixels(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Light => 4,
            Self::Medium => 8,
            Self::Strong => 14,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct WindowSize {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 820,
            maximized: false,
        }
    }
}

impl WindowSize {
    pub const MINIMUM_WIDTH: i32 = 480;
    pub const MINIMUM_HEIGHT: i32 = 400;

    pub fn sanitized(self) -> Self {
        let fallback = Self::default();
        Self {
            width: if self.width >= Self::MINIMUM_WIDTH {
                self.width
            } else {
                fallback.width
            },
            height: if self.height >= Self::MINIMUM_HEIGHT {
                self.height
            } else {
                fallback.height
            },
            maximized: self.maximized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
#[allow(clippy::struct_excessive_bools)]
pub struct Preferences {
    pub theme: ThemeChoice,
    pub close_action: CloseAction,
    pub start_in_background: bool,
    pub launch_at_login: bool,
    pub notifications: bool,
    pub notification_detail: NotificationDetail,
    pub unread_badge: bool,
    pub tray_icon: bool,
    pub tray_style: TrayStyle,
    pub lock_on_start: bool,
    pub lock_when_hidden: bool,
    pub lock_after_minutes: u32,
    pub allow_microphone: bool,
    pub allow_camera: bool,
    pub allow_screen_sharing: bool,
    pub veil: Veil,
    pub spell_check: bool,
    pub spell_languages: Vec<String>,
    pub proxy: ProxySettings,
    pub hardware_acceleration: bool,
    pub window: WindowSize,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::System,
            close_action: CloseAction::KeepInBackground,
            start_in_background: false,
            launch_at_login: false,
            notifications: true,
            notification_detail: NotificationDetail::SenderAndMessage,
            unread_badge: true,
            tray_icon: true,
            tray_style: TrayStyle::Light,
            lock_on_start: false,
            lock_when_hidden: false,
            lock_after_minutes: 0,
            allow_microphone: true,
            allow_camera: true,
            allow_screen_sharing: true,
            veil: Veil::Off,
            spell_check: true,
            spell_languages: crate::spelling::default_languages(),
            proxy: ProxySettings::default(),
            hardware_acceleration: true,
            window: WindowSize::default(),
        }
    }
}

impl Preferences {
    pub fn from_toml(source: &str) -> Self {
        toml::from_str(source).unwrap_or_else(|error| {
            tracing::warn!(%error, "preferences file unreadable, using defaults");
            Self::default()
        })
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }
}

type Listener = Box<dyn Fn(&Preferences, &Preferences)>;

pub struct PreferenceStore {
    location: PathBuf,
    current: RefCell<Preferences>,
    listeners: RefCell<Vec<Listener>>,
}

impl PreferenceStore {
    pub fn open() -> Rc<Self> {
        let profile = Profile::from_environment();
        let location = glib::user_config_dir()
            .join(APP_ID)
            .join(format!("{}.toml", profile.engine_folder()));
        let current = std::fs::read_to_string(&location)
            .map_or_default(|source| Preferences::from_toml(&source));
        Rc::new(Self {
            location,
            current: RefCell::new(current),
            listeners: RefCell::default(),
        })
    }

    pub fn snapshot(&self) -> Preferences {
        self.current.borrow().clone()
    }

    pub fn update(&self, change: impl FnOnce(&mut Preferences)) {
        let previous = self.snapshot();
        let mut next = previous.clone();
        change(&mut next);
        if next == previous {
            return;
        }
        self.current.replace(next.clone());
        self.persist(&next);
        for listener in self.listeners.borrow().iter() {
            listener(&previous, &next);
        }
    }

    pub fn subscribe(&self, listener: impl Fn(&Preferences, &Preferences) + 'static) {
        self.listeners.borrow_mut().push(Box::new(listener));
    }

    fn persist(&self, preferences: &Preferences) {
        let written = self
            .location
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&self.location, preferences.to_toml()));
        if let Err(error) = written {
            tracing::warn!(%error, path = %self.location.display(), "cannot save preferences");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CloseAction, NotificationDetail, Preferences, ThemeChoice, Veil, WindowSize};

    #[test]
    fn rejects_unusable_window_sizes() {
        let tiny = WindowSize {
            width: 10,
            height: 5000,
            maximized: true,
        };
        assert_eq!(
            tiny.sanitized(),
            WindowSize {
                width: 1200,
                height: 5000,
                maximized: true
            }
        );
    }

    #[test]
    fn veil_levels_grow() {
        assert_eq!(Veil::Off.pixels(), 0);
        assert!(Veil::Light.pixels() < Veil::Medium.pixels());
        assert!(Veil::Medium.pixels() < Veil::Strong.pixels());
    }

    #[test]
    fn round_trips_through_toml() {
        let preferences = Preferences {
            theme: ThemeChoice::Dark,
            close_action: CloseAction::Quit,
            notification_detail: NotificationDetail::Nothing,
            ..Preferences::default()
        };
        assert_eq!(Preferences::from_toml(&preferences.to_toml()), preferences);
    }

    #[test]
    fn fills_missing_fields_with_defaults() {
        let preferences = Preferences::from_toml("theme = \"light\"\n");
        assert_eq!(preferences.theme, ThemeChoice::Light);
        assert!(preferences.notifications);
        assert_eq!(preferences.close_action, CloseAction::KeepInBackground);
    }

    #[test]
    fn survives_corrupted_files() {
        assert_eq!(
            Preferences::from_toml("theme = 42 ]["),
            Preferences::default()
        );
    }
}
