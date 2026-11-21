use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::config::{APP_ID, Profile};

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
    use super::{CloseAction, NotificationDetail, Preferences, ThemeChoice};

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
