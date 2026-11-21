use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::config::APP_ID;

const SIGNAL_PATH: &str = "/io/github/balthier82/Wazzup/Launcher";
const SIGNAL_INTERFACE: &str = "com.canonical.Unity.LauncherEntry";

pub struct LauncherBadge {
    connection: Option<gio::DBusConnection>,
    application_uri: String,
}

impl LauncherBadge {
    pub fn new(application: &adw::Application) -> Self {
        Self {
            connection: application.dbus_connection(),
            application_uri: format!("application://{APP_ID}.desktop"),
        }
    }

    pub fn show(&self, count: u32) {
        let Some(connection) = &self.connection else {
            return;
        };
        let properties = glib::VariantDict::new(None);
        properties.insert_value("count", &i64::from(count).to_variant());
        properties.insert_value("count-visible", &(count > 0).to_variant());
        let parameters =
            glib::Variant::tuple_from_iter([self.application_uri.to_variant(), properties.end()]);
        if let Err(error) = connection.emit_signal(
            None,
            SIGNAL_PATH,
            SIGNAL_INTERFACE,
            "Update",
            Some(&parameters),
        ) {
            tracing::debug!(%error, "launcher badge not delivered");
        }
    }
}
