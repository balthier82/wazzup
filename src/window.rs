use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::browser::{BrowserSurface, SurfaceObserver};
use crate::call_window::CallWindow;
use crate::config::{APP_NAME, HOME_URL};
use crate::unread::UnreadCount;

pub struct MainWindow {
    application: adw::Application,
    window: adw::ApplicationWindow,
    title: adw::WindowTitle,
    surface: BrowserSurface,
}

impl MainWindow {
    pub fn new(application: &adw::Application) -> Rc<Self> {
        let main_window = Rc::new_cyclic(|this: &Weak<Self>| {
            let observer: Weak<dyn SurfaceObserver> = this.clone();
            let surface = BrowserSurface::new(observer);
            let title = adw::WindowTitle::new(APP_NAME, "");
            let header = adw::HeaderBar::builder().title_widget(&title).build();
            header.pack_start(
                &gtk::Button::builder()
                    .icon_name("view-refresh-symbolic")
                    .tooltip_text("Reload")
                    .action_name("win.reload")
                    .build(),
            );
            let toolbar = adw::ToolbarView::new();
            toolbar.add_top_bar(&header);
            toolbar.set_content(Some(surface.widget()));
            let window = adw::ApplicationWindow::builder()
                .application(application)
                .title(APP_NAME)
                .default_width(1200)
                .default_height(820)
                .content(&toolbar)
                .build();
            Self {
                application: application.clone(),
                window,
                title,
                surface,
            }
        });
        main_window.install_actions();
        main_window.guard_close();
        main_window.surface.load(HOME_URL);
        main_window
    }

    pub fn present(&self) {
        self.window.present();
        self.surface.widget().grab_focus();
    }

    pub fn open_notification(&self, id: u64) {
        self.present();
        self.surface.activate_notification(id);
    }

    fn install_actions(&self) {
        let surface = self.surface.clone();
        let reload = gio::ActionEntry::builder("reload")
            .activate(move |_: &adw::ApplicationWindow, _, _| surface.reload())
            .build();
        self.window.add_action_entries([reload]);
    }

    fn guard_close(&self) {
        let surface = self.surface.clone();
        self.window.connect_close_request(move |_| {
            if surface.request_close() {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
}

impl SurfaceObserver for MainWindow {
    fn title_changed(&self, title: &str) {
        let count = UnreadCount::from_page_title(title);
        let subtitle = match count.value() {
            0 => String::new(),
            1 => "1 unread chat".to_owned(),
            many => format!("{many} unread chats"),
        };
        self.title.set_subtitle(&subtitle);
        let window_title = if count.is_zero() {
            APP_NAME.to_owned()
        } else {
            format!("({}) {APP_NAME}", count.value())
        };
        self.window.set_title(Some(&window_title));
    }

    fn notification_posted(&self, id: u64, title: &str, body: &str) {
        let notification = gio::Notification::new(title);
        notification.set_body(Some(body));
        notification
            .set_default_action_and_target_value("app.open-notification", Some(&id.to_variant()));
        self.application
            .send_notification(Some(&notification_key(id)), &notification);
    }

    fn notification_withdrawn(&self, id: u64) {
        self.application
            .withdraw_notification(&notification_key(id));
    }

    fn surface_closed(&self) {
        self.window.destroy();
    }

    fn companion_requested(&self, width: i32, height: i32) -> Option<BrowserSurface> {
        Some(CallWindow::open(&self.application, width, height))
    }
}

fn notification_key(id: u64) -> String {
    format!("message-{id}")
}
