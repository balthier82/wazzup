use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::browser::{BrowserSurface, SurfaceObserver};
use crate::call_window::CallWindow;
use crate::config::{APP_NAME, HOME_URL};
use crate::desktop::LauncherBadge;
use crate::preferences::{CloseAction, NotificationDetail, PreferenceStore};
use crate::tray::TrayIcon;
use crate::unread::UnreadCount;

pub struct MainWindow {
    application: adw::Application,
    window: adw::ApplicationWindow,
    title: adw::WindowTitle,
    surface: BrowserSurface,
    store: Rc<PreferenceStore>,
    badge: LauncherBadge,
    tray: Rc<RefCell<TrayIcon>>,
    unread: Cell<UnreadCount>,
    quitting: Cell<bool>,
}

impl MainWindow {
    pub fn new(
        application: &adw::Application,
        store: &Rc<PreferenceStore>,
        tray: &Rc<RefCell<TrayIcon>>,
    ) -> Rc<Self> {
        let main_window = Rc::new_cyclic(|this: &Weak<Self>| {
            let observer: Weak<dyn SurfaceObserver> = this.clone();
            let surface = BrowserSurface::new(observer);
            let title = adw::WindowTitle::new(APP_NAME, "");
            let toolbar = adw::ToolbarView::new();
            toolbar.add_top_bar(&header_bar(&title));
            toolbar.set_content(Some(surface.widget()));
            let window = adw::ApplicationWindow::builder()
                .application(application)
                .title(APP_NAME)
                .default_width(1200)
                .default_height(820)
                .content(&toolbar)
                .hide_on_close(false)
                .build();
            Self {
                application: application.clone(),
                window,
                title,
                surface,
                store: Rc::clone(store),
                badge: LauncherBadge::new(application),
                tray: Rc::clone(tray),
                unread: Cell::default(),
                quitting: Cell::new(false),
            }
        });
        main_window.install_actions();
        main_window.guard_close();
        main_window.follow_preferences();
        main_window.surface.load(HOME_URL);
        main_window
    }

    pub fn present(&self) {
        self.window.present();
        self.surface.widget().grab_focus();
    }

    pub fn toggle(&self) {
        if self.window.is_visible() && self.window.is_active() {
            self.window.set_visible(false);
        } else {
            self.present();
        }
    }

    pub fn open_chat(&self, address: &str) {
        self.surface.navigate(address);
        self.present();
    }

    pub fn open_notification(&self, id: u64) {
        self.present();
        self.surface.activate_notification(id);
    }

    pub fn show_preferences(&self) {
        crate::preferences_dialog::build(&self.store).present(Some(&self.window));
    }

    pub fn show_about(&self) {
        adw::AboutDialog::builder()
            .application_name(APP_NAME)
            .application_icon(crate::config::APP_ID)
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("balthier82")
            .website("https://github.com/balthier82/wazzup")
            .issue_url("https://github.com/balthier82/wazzup/issues")
            .comments("WhatsApp on your GNOME desktop")
            .build()
            .present(Some(&self.window));
    }

    pub fn quit(&self) {
        self.quitting.set(true);
        self.badge.show(0);
        for window in self.application.windows() {
            if window != *self.window.upcast_ref::<gtk::Window>() {
                window.close();
            }
        }
        if !self.surface.request_close() {
            self.window.destroy();
        }
    }

    fn install_actions(&self) {
        let surface = self.surface.clone();
        let reload = gio::ActionEntry::builder("reload")
            .activate(move |_: &adw::ApplicationWindow, _, _| surface.reload())
            .build();
        self.window.add_action_entries([reload]);
    }

    fn guard_close(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.window.connect_close_request(move |window| {
            let Some(this) = this.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let keep_running = this.store.snapshot().close_action == CloseAction::KeepInBackground;
            if keep_running && !this.quitting.get() {
                window.set_visible(false);
            } else {
                this.quit();
            }
            glib::Propagation::Stop
        });
    }

    fn follow_preferences(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.store.subscribe(move |_, current| {
            if let Some(this) = this.upgrade() {
                let count = if current.unread_badge {
                    this.unread.get().value()
                } else {
                    0
                };
                this.badge.show(count);
            }
        });
    }

    fn describe(&self, title: &str, body: &str) -> Option<gio::Notification> {
        let preferences = self.store.snapshot();
        if !preferences.notifications {
            return None;
        }
        let notification = match preferences.notification_detail {
            NotificationDetail::SenderAndMessage => {
                let notification = gio::Notification::new(title);
                notification.set_body(Some(body));
                notification
            }
            NotificationDetail::SenderOnly => {
                let notification = gio::Notification::new(title);
                notification.set_body(Some("New message"));
                notification
            }
            NotificationDetail::Nothing => gio::Notification::new("New message"),
        };
        Some(notification)
    }
}

fn header_bar(title: &adw::WindowTitle) -> adw::HeaderBar {
    let header = adw::HeaderBar::builder().title_widget(title).build();
    header.pack_start(
        &gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload")
            .action_name("win.reload")
            .build(),
    );
    let menu = gio::Menu::new();
    let sections = gio::Menu::new();
    sections.append(Some("Preferences"), Some("app.preferences"));
    sections.append(Some("About Wazzup"), Some("app.about"));
    menu.append_section(None, &sections);
    let ending = gio::Menu::new();
    ending.append(Some("Quit"), Some("app.quit"));
    menu.append_section(None, &ending);
    header.pack_end(
        &gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Main menu")
            .menu_model(&menu)
            .primary(true)
            .build(),
    );
    header
}

impl SurfaceObserver for MainWindow {
    fn title_changed(&self, title: &str) {
        let count = UnreadCount::from_page_title(title);
        self.unread.set(count);
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
        self.tray.borrow_mut().set_unread(count.value());
        if self.store.snapshot().unread_badge {
            self.badge.show(count.value());
        }
    }

    fn notification_posted(&self, id: u64, title: &str, body: &str) {
        let Some(notification) = self.describe(title, body) else {
            return;
        };
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
