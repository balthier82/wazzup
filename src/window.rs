use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::browser::{BrowserSurface, SurfaceObserver};
use crate::call_window::{CallWindow, MediaPolicy};
use crate::config::{APP_NAME, HOME_URL};
use crate::desktop::LauncherBadge;
use crate::downloads::{DownloadsPanel, unused_path};
use crate::engine::{DownloadControl, DownloadProgress, MediaAccess};
use crate::i18n::{t, t_count, t_with};
use crate::lock::{AppLock, LockScreen};
use crate::preferences::{
    CloseAction, NotificationDetail, PreferenceStore, Preferences, WindowSize,
};
use crate::proxy::ProxyPassword;
use crate::recovery::{ConnectionWatchdog, CrashBudget, Verdict};
use crate::tray::TrayIcon;
use crate::unread::UnreadCount;

const APP_PAGE: &str = "app";
const LOCK_PAGE: &str = "lock";
const FAILURE_PAGE: &str = "failure";
const TICK: Duration = Duration::from_secs(1);
const IDLE_CHECK: Duration = Duration::from_secs(20);

pub struct MainWindow {
    this: Weak<Self>,
    application: adw::Application,
    window: adw::ApplicationWindow,
    pages: gtk::Stack,
    title: adw::WindowTitle,
    surface: BrowserSurface,
    store: Rc<PreferenceStore>,
    lock: Rc<AppLock>,
    lock_screen: LockScreen,
    downloads: Rc<DownloadsPanel>,
    badge: LauncherBadge,
    tray: Rc<RefCell<TrayIcon>>,
    unread: Cell<UnreadCount>,
    last_activity: Cell<Instant>,
    quitting: Cell<bool>,
    banner: adw::Banner,
    failure: adw::StatusPage,
    failed: Cell<bool>,
    watchdog: RefCell<ConnectionWatchdog>,
    crashes: RefCell<CrashBudget>,
    proxy_password: Rc<ProxyPassword>,
}

pub struct Shared<'a> {
    pub store: &'a Rc<PreferenceStore>,
    pub lock: &'a Rc<AppLock>,
    pub tray: &'a Rc<RefCell<TrayIcon>>,
    pub proxy_password: &'a Rc<ProxyPassword>,
}

impl MainWindow {
    pub fn new(application: &adw::Application, shared: &Shared<'_>) -> Rc<Self> {
        let remembered = shared.store.snapshot().window.sanitized();
        let main_window = Rc::new_cyclic(|this: &Weak<Self>| {
            let observer: Weak<dyn SurfaceObserver> = this.clone();
            let surface = BrowserSurface::new(observer);
            let downloads = DownloadsPanel::new();
            let title = adw::WindowTitle::new(APP_NAME, "");
            let toolbar = adw::ToolbarView::new();
            toolbar.add_top_bar(&header_bar(&title, downloads.button()));
            let banner = adw::Banner::builder()
                .button_label(t("Retry Now"))
                .revealed(false)
                .build();
            toolbar.add_top_bar(&banner);
            toolbar.set_content(Some(surface.widget()));
            let retry = gtk::Button::builder()
                .label(t("Try Again"))
                .halign(gtk::Align::Center)
                .css_classes(["suggested-action", "pill"])
                .build();
            let failure = adw::StatusPage::builder()
                .icon_name("network-offline-symbolic")
                .child(&retry)
                .build();
            let failure_view = adw::ToolbarView::new();
            failure_view.add_top_bar(&adw::HeaderBar::new());
            failure_view.set_content(Some(&failure));
            let lock_screen = LockScreen::new(shared.lock);
            let pages = gtk::Stack::builder()
                .transition_type(gtk::StackTransitionType::Crossfade)
                .build();
            pages.add_named(&toolbar, Some(APP_PAGE));
            pages.add_named(lock_screen.widget(), Some(LOCK_PAGE));
            pages.add_named(&failure_view, Some(FAILURE_PAGE));
            let weak = this.clone();
            retry.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.reload_page();
                }
            });
            let weak = this.clone();
            banner.connect_button_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.watchdog.borrow_mut().retry_now(Instant::now());
                }
            });
            let window = adw::ApplicationWindow::builder()
                .application(application)
                .title(APP_NAME)
                .default_width(remembered.width)
                .default_height(remembered.height)
                .maximized(remembered.maximized)
                .width_request(WindowSize::MINIMUM_WIDTH)
                .height_request(WindowSize::MINIMUM_HEIGHT)
                .content(&pages)
                .build();
            Self {
                this: this.clone(),
                application: application.clone(),
                window,
                pages,
                title,
                surface,
                store: Rc::clone(shared.store),
                lock: Rc::clone(shared.lock),
                lock_screen,
                downloads,
                badge: LauncherBadge::new(application),
                tray: Rc::clone(shared.tray),
                unread: Cell::default(),
                last_activity: Cell::new(Instant::now()),
                quitting: Cell::new(false),
                banner,
                failure,
                failed: Cell::new(false),
                watchdog: RefCell::default(),
                crashes: RefCell::default(),
                proxy_password: Rc::clone(shared.proxy_password),
            }
        });
        main_window.install_actions();
        main_window.guard_close();
        main_window.follow_preferences();
        main_window.follow_lock();
        main_window.watch_activity();
        main_window.watch_connection();
        if main_window.store.snapshot().lock_on_start {
            main_window.lock.lock();
        }
        main_window.surface.load(HOME_URL);
        main_window
    }

    pub fn present(&self) {
        self.window.present();
        if self.lock.is_locked() {
            self.lock_screen.reset();
        } else {
            self.surface.widget().grab_focus();
        }
    }

    pub fn toggle(&self) {
        if self.window.is_visible() && self.window.is_active() {
            self.remember_size();
            self.window.set_visible(false);
        } else {
            self.present();
        }
    }

    pub fn open_chat(&self, address: &str) {
        if !self.lock.is_locked() {
            self.surface.navigate(address);
        }
        self.present();
    }

    pub fn open_notification(&self, id: u64) {
        self.present();
        if !self.lock.is_locked() {
            self.surface.activate_notification(id);
        }
    }

    pub fn show_preferences(&self) {
        self.present();
        if !self.lock.is_locked() {
            crate::preferences_dialog::build(&self.store, &self.lock, &self.proxy_password)
                .present(Some(&self.window));
        }
    }

    pub fn show_about(&self) {
        if self.lock.is_locked() {
            self.present();
            return;
        }
        adw::AboutDialog::builder()
            .application_name(APP_NAME)
            .application_icon(crate::config::APP_ID)
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("balthier82")
            .website("https://github.com/balthier82/wazzup")
            .issue_url("https://github.com/balthier82/wazzup/issues")
            .comments(t("WhatsApp on your GNOME desktop"))
            .debug_info(
                crate::diagnostics::Scrubber::for_this_session()
                    .clean(&crate::diagnostics::environment()),
            )
            .debug_info_filename("wazzup-debug.txt")
            .build()
            .present(Some(&self.window));
    }

    pub fn show_shortcuts(&self) {
        if !self.lock.is_locked() {
            crate::shortcuts::window(&self.window).present();
        }
    }

    pub fn show_report(&self) {
        self.present();
        if !self.lock.is_locked() {
            crate::report_dialog::present(&self.window);
        }
    }

    pub fn lock_now(&self) {
        self.lock.lock();
    }

    pub fn quit(&self) {
        self.remember_size();
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

    fn media_policy(self: &Rc<Self>) -> MediaPolicy {
        let this = Rc::downgrade(self);
        Rc::new(move || {
            this.upgrade()
                .map_or_else(MediaAccess::default, |this| this.current_media_access())
        })
    }

    fn current_media_access(&self) -> MediaAccess {
        if self.lock.is_locked() {
            return MediaAccess::default();
        }
        let preferences = self.store.snapshot();
        MediaAccess {
            microphone: preferences.allow_microphone,
            camera: preferences.allow_camera,
            screen: preferences.allow_screen_sharing,
        }
    }

    fn install_actions(&self) {
        let surface = self.surface.clone();
        let lock = Rc::clone(&self.lock);
        let reload = gio::ActionEntry::builder("reload")
            .activate(move |_: &adw::ApplicationWindow, _, _| {
                if !lock.is_locked() {
                    surface.reload();
                }
            })
            .build();
        self.window.add_action_entries([reload]);
    }

    fn remember_size(&self) {
        let (width, height) = self.window.default_size();
        let size = WindowSize {
            width,
            height,
            maximized: self.window.is_maximized(),
        };
        self.store.update(|preferences| preferences.window = size);
    }

    fn guard_close(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.window.connect_close_request(move |window| {
            let Some(this) = this.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let keep_running = this.store.snapshot().close_action == CloseAction::KeepInBackground;
            if keep_running && !this.quitting.get() {
                this.remember_size();
                window.set_visible(false);
            } else {
                this.quit();
            }
            glib::Propagation::Stop
        });
        let this = Rc::downgrade(self);
        self.window.connect_visible_notify(move |window| {
            if let Some(this) = this.upgrade()
                && !window.is_visible()
                && this.store.snapshot().lock_when_hidden
            {
                this.lock.lock();
            }
        });
    }

    fn follow_preferences(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.store.subscribe(move |previous, current| {
            let Some(this) = this.upgrade() else {
                return;
            };
            let count = if current.unread_badge {
                this.unread.get().value()
            } else {
                0
            };
            this.badge.show(count);
            if previous.veil != current.veil {
                this.surface.apply_veil(current.veil.pixels());
            }
        });
    }

    fn follow_lock(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        self.lock.subscribe(move |locked| {
            let Some(this) = this.upgrade() else {
                return;
            };
            if locked {
                if let Some(dialog) = this.window.visible_dialog() {
                    dialog.force_close();
                }
                this.show_current_page();
                this.lock_screen.reset();
            } else {
                this.last_activity.set(Instant::now());
                this.show_current_page();
                this.surface.widget().grab_focus();
            }
        });
        if self.lock.is_locked() {
            self.pages.set_visible_child_name(LOCK_PAGE);
        }
    }

    fn watch_activity(self: &Rc<Self>) {
        let keys = gtk::EventControllerKey::new();
        let pointer = gtk::EventControllerMotion::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        pointer.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, _, _, _| {
            if let Some(this) = this.upgrade() {
                this.last_activity.set(Instant::now());
            }
            glib::Propagation::Proceed
        });
        let this = Rc::downgrade(self);
        pointer.connect_motion(move |_, _, _| {
            if let Some(this) = this.upgrade() {
                this.last_activity.set(Instant::now());
            }
        });
        self.window.add_controller(keys);
        self.window.add_controller(pointer);

        let this = Rc::downgrade(self);
        glib::timeout_add_local(IDLE_CHECK, move || {
            let Some(this) = this.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let minutes = this.store.snapshot().lock_after_minutes;
            let limit = Duration::from_secs(u64::from(minutes) * 60);
            if minutes > 0 && this.last_activity.get().elapsed() >= limit {
                this.lock.lock();
            }
            glib::ControlFlow::Continue
        });
    }

    fn show_current_page(&self) {
        let page = if self.lock.is_locked() {
            LOCK_PAGE
        } else if self.failed.get() {
            FAILURE_PAGE
        } else {
            APP_PAGE
        };
        self.pages.set_visible_child_name(page);
    }

    fn reload_page(&self) {
        if self.failed.replace(false) {
            self.show_current_page();
            self.surface.navigate(HOME_URL);
        } else {
            self.surface.reload();
        }
    }

    fn show_failure(&self, icon: &str, title: &str, description: &str) {
        self.failure.set_icon_name(Some(icon));
        self.failure.set_title(title);
        self.failure.set_description(Some(description));
        self.failed.set(true);
        self.show_current_page();
    }

    fn watch_connection(self: &Rc<Self>) {
        let this = Rc::downgrade(self);
        gio::NetworkMonitor::default().connect_network_changed(move |_, available| {
            if let Some(this) = this.upgrade()
                && available
            {
                this.watchdog.borrow_mut().network_returned(Instant::now());
            }
        });
        let this = Rc::downgrade(self);
        glib::timeout_add_local(TICK, move || {
            let Some(this) = this.upgrade() else {
                return glib::ControlFlow::Break;
            };
            this.tick();
            glib::ControlFlow::Continue
        });
    }

    fn tick(&self) {
        let now = Instant::now();
        let verdict = self.watchdog.borrow_mut().poll(now);
        if verdict == Verdict::Reload {
            tracing::info!("reloading after lost connection");
            self.reload_page();
        }
        let watchdog = self.watchdog.borrow();
        let show_banner = watchdog.is_offline() && !self.failed.get();
        self.banner.set_revealed(show_banner);
        if let Some(remaining) = watchdog.countdown(now).filter(|_| show_banner) {
            self.banner.set_title(&t_with(
                "Connection lost. Retrying in {seconds} s",
                &[("seconds", &remaining.as_secs().max(1))],
            ));
        }
    }

    fn describe(&self, title: &str, body: &str) -> Option<gio::Notification> {
        let preferences: Preferences = self.store.snapshot();
        if !preferences.notifications {
            return None;
        }
        if self.lock.is_locked() {
            return Some(gio::Notification::new(&t("New message")));
        }
        let notification = match preferences.notification_detail {
            NotificationDetail::SenderAndMessage => {
                let notification = gio::Notification::new(title);
                notification.set_body(Some(body));
                notification
            }
            NotificationDetail::SenderOnly => {
                let notification = gio::Notification::new(title);
                notification.set_body(Some(&t("New message")));
                notification
            }
            NotificationDetail::Nothing => gio::Notification::new(&t("New message")),
        };
        Some(notification)
    }
}

fn header_bar(title: &adw::WindowTitle, downloads: &gtk::MenuButton) -> adw::HeaderBar {
    let header = adw::HeaderBar::builder().title_widget(title).build();
    header.pack_start(
        &gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(t("Reload"))
            .action_name("win.reload")
            .build(),
    );
    let menu = gio::Menu::new();
    let sections = gio::Menu::new();
    sections.append(Some(&t("Lock")), Some("app.lock"));
    sections.append(Some(&t("Preferences")), Some("app.preferences"));
    sections.append(Some(&t("Keyboard Shortcuts")), Some("app.shortcuts"));
    sections.append(Some(&t("Report a Problem…")), Some("app.report"));
    sections.append(Some(&t("About Wazzup")), Some("app.about"));
    menu.append_section(None, &sections);
    let ending = gio::Menu::new();
    ending.append(Some(&t("Quit")), Some("app.quit"));
    menu.append_section(None, &ending);
    header.pack_end(
        &gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text(t("Main menu"))
            .menu_model(&menu)
            .primary(true)
            .build(),
    );
    header.pack_end(downloads);
    header
}

impl SurfaceObserver for MainWindow {
    fn title_changed(&self, title: &str) {
        let count = UnreadCount::from_page_title(title);
        self.unread.set(count);
        let subtitle = if count.is_zero() {
            String::new()
        } else {
            t_count(
                "{count} unread chat",
                "{count} unread chats",
                u64::from(count.value()),
            )
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
        if self.lock.is_locked() {
            return None;
        }
        let policy = self.this.upgrade()?.media_policy();
        Some(CallWindow::open(&self.application, (width, height), policy))
    }

    fn page_loaded(&self) {
        self.surface.apply_veil(self.store.snapshot().veil.pixels());
    }

    fn media_allowed(&self) -> MediaAccess {
        self.current_media_access()
    }

    fn download_target(&self, suggested_name: &str) -> Option<PathBuf> {
        if self.lock.is_locked() {
            return None;
        }
        let folder =
            glib::user_special_dir(glib::UserDirectory::Downloads).unwrap_or_else(glib::home_dir);
        Some(unused_path(&folder, suggested_name))
    }

    fn files_allowed(&self) -> bool {
        !self.lock.is_locked()
    }

    fn proxy_credentials(&self) -> Option<(String, String)> {
        let username = self.store.snapshot().proxy.username;
        let password = self.proxy_password.current()?;
        (!username.is_empty()).then_some((username, password))
    }

    fn connection_changed(&self, online: bool) {
        tracing::info!(online, "WhatsApp connection changed");
        if online {
            self.watchdog.borrow_mut().went_online();
            self.crashes.borrow_mut().forgive();
            if self.failed.replace(false) {
                self.show_current_page();
            }
        } else {
            self.watchdog.borrow_mut().went_offline(Instant::now());
        }
        self.tick();
    }

    fn page_crashed(&self) {
        let delay = self.crashes.borrow_mut().record(Instant::now());
        match delay {
            Some(delay) => {
                let surface = self.surface.clone();
                glib::timeout_add_local_once(delay, move || surface.reload());
            }
            None => self.show_failure(
                "dialog-error-symbolic",
                &t("WhatsApp Stopped Working"),
                &t("The page closed unexpectedly several times."),
            ),
        }
    }

    fn load_failed(&self, description: &str) {
        self.watchdog.borrow_mut().went_offline(Instant::now());
        let detail = if description.is_empty() {
            t("Check your internet connection. Wazzup will try again automatically.")
        } else {
            t_with(
                "{description}. Wazzup will try again automatically.",
                &[("description", &description)],
            )
        };
        self.show_failure(
            "network-offline-symbolic",
            &t("Can’t Reach WhatsApp"),
            &detail,
        );
    }

    fn download_progress(&self, progress: DownloadProgress, control: Option<DownloadControl>) {
        self.downloads.track(&progress, control);
    }
}

fn notification_key(id: u64) -> String {
    format!("message-{id}")
}
