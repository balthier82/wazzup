use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::chat_link::ChatLink;
use crate::config::Profile;
use crate::desktop::{Autostart, XdgAutostart};
use crate::engine::{EngineOptions, ProxySwitch};
use crate::launch::LaunchRequest;
use crate::lock::AppLock;
use crate::preferences::{PreferenceStore, Preferences, ThemeChoice};
use crate::proxy::{ProxyPassword, ProxyRoute};
use crate::tray::{TrayCommand, TrayIcon};
use crate::window::{MainWindow, Shared};

pub struct WazzupApplication {
    inner: adw::Application,
    window: Rc<OnceCell<Rc<MainWindow>>>,
    store: Rc<PreferenceStore>,
    tray: Rc<RefCell<TrayIcon>>,
    lock: Rc<AppLock>,
    proxy_password: Rc<OnceCell<Rc<ProxyPassword>>>,
    tray_commands: async_channel::Receiver<TrayCommand>,
}

impl WazzupApplication {
    pub fn new() -> Self {
        let inner = adw::Application::builder()
            .application_id(Profile::from_environment().application_id())
            .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .build();
        let (tray, tray_commands) = TrayIcon::new();
        let application = Self {
            inner,
            window: Rc::default(),
            store: PreferenceStore::open(),
            tray: Rc::new(RefCell::new(tray)),
            lock: AppLock::open(),
            proxy_password: Rc::default(),
            tray_commands,
        };
        application.wire();
        application
    }

    pub fn engine_options(&self, gpu_failed_before: bool) -> EngineOptions {
        let preferences = self.store.snapshot();
        if gpu_failed_before && preferences.hardware_acceleration {
            tracing::warn!("previous starts failed, switching to software rendering");
            self.store
                .update(|preferences| preferences.hardware_acceleration = false);
        }
        let proxy = match preferences.proxy.route() {
            ProxyRoute::System => ProxySwitch::System,
            ProxyRoute::Direct => ProxySwitch::Direct,
            ProxyRoute::Through(server) => ProxySwitch::Server(server),
        };
        EngineOptions {
            proxy,
            spell_check: preferences.spell_check,
            spell_languages: preferences.spell_languages,
            software_rendering: gpu_failed_before || !preferences.hardware_acceleration,
        }
    }

    pub fn register(&self) -> Result<bool, glib::Error> {
        self.inner.register(None::<&gio::Cancellable>)?;
        Ok(self.inner.is_remote())
    }

    pub fn run(&self) -> glib::ExitCode {
        self.inner.run()
    }

    fn wire(&self) {
        let store = Rc::clone(&self.store);
        let tray = Rc::clone(&self.tray);
        let password = Rc::clone(&self.proxy_password);
        self.inner.connect_startup(move |_| {
            password.get_or_init(ProxyPassword::load);
            let preferences = store.snapshot();
            apply_theme(&preferences);
            sync_autostart(preferences.launch_at_login);
            sync_tray(&tray, &preferences);
            let tray = Rc::clone(&tray);
            store.subscribe(move |previous, current| {
                if previous.theme != current.theme {
                    apply_theme(current);
                }
                if previous.launch_at_login != current.launch_at_login {
                    sync_autostart(current.launch_at_login);
                }
                sync_tray(&tray, current);
            });
        });
        self.listen_to_tray();

        let window = Rc::clone(&self.window);
        let store = Rc::clone(&self.store);
        let tray = Rc::clone(&self.tray);
        let lock = Rc::clone(&self.lock);
        let password = Rc::clone(&self.proxy_password);
        self.inner
            .connect_command_line(move |application, command_line| {
                let arguments: Vec<String> = command_line
                    .arguments()
                    .iter()
                    .skip(1)
                    .map(|argument| argument.to_string_lossy().into_owned())
                    .collect();
                let request = LaunchRequest::parse(&arguments);
                let proxy_password = password.get_or_init(ProxyPassword::load);
                let shared = Shared {
                    store: &store,
                    lock: &lock,
                    tray: &tray,
                    proxy_password,
                };
                handle(application, &window, &shared, &request, command_line);
                glib::ExitCode::SUCCESS
            });

        self.install_actions();
    }

    fn listen_to_tray(&self) {
        let commands = self.tray_commands.clone();
        let window = Rc::clone(&self.window);
        let application = self.inner.clone();
        glib::spawn_future_local(async move {
            while let Ok(command) = commands.recv().await {
                let Some(main_window) = window.get() else {
                    if command == TrayCommand::Quit {
                        application.quit();
                    }
                    continue;
                };
                match command {
                    TrayCommand::ToggleWindow => main_window.toggle(),
                    TrayCommand::OpenWindow => main_window.present(),
                    TrayCommand::OpenPreferences => {
                        main_window.present();
                        main_window.show_preferences();
                    }
                    TrayCommand::Quit => main_window.quit(),
                }
            }
        });
    }

    fn install_actions(&self) {
        let window = Rc::clone(&self.window);
        let present = gio::ActionEntry::builder("present")
            .activate(move |_: &adw::Application, _, _| {
                if let Some(main_window) = window.get() {
                    main_window.present();
                }
            })
            .build();

        let window = Rc::clone(&self.window);
        let open_notification = gio::ActionEntry::builder("open-notification")
            .parameter_type(Some(&u64::static_variant_type()))
            .activate(move |_: &adw::Application, _, parameter| {
                let id = parameter.and_then(u64::from_variant);
                if let (Some(main_window), Some(id)) = (window.get(), id) {
                    main_window.open_notification(id);
                }
            })
            .build();

        let window = Rc::clone(&self.window);
        let preferences = gio::ActionEntry::builder("preferences")
            .activate(move |_: &adw::Application, _, _| {
                if let Some(main_window) = window.get() {
                    main_window.present();
                    main_window.show_preferences();
                }
            })
            .build();

        let window = Rc::clone(&self.window);
        let about = gio::ActionEntry::builder("about")
            .activate(move |_: &adw::Application, _, _| {
                if let Some(main_window) = window.get() {
                    main_window.show_about();
                }
            })
            .build();

        let window = Rc::clone(&self.window);
        let lock = gio::ActionEntry::builder("lock")
            .activate(move |_: &adw::Application, _, _| {
                if let Some(main_window) = window.get() {
                    main_window.lock_now();
                }
            })
            .build();

        let window = Rc::clone(&self.window);
        let quit = gio::ActionEntry::builder("quit")
            .activate(
                move |application: &adw::Application, _, _| match window.get() {
                    Some(main_window) => main_window.quit(),
                    None => application.quit(),
                },
            )
            .build();

        self.inner
            .add_action_entries([present, open_notification, preferences, about, lock, quit]);
        self.inner
            .set_accels_for_action("app.quit", &["<Control>q"]);
        self.inner
            .set_accels_for_action("app.preferences", &["<Control>comma"]);
        self.inner
            .set_accels_for_action("app.lock", &["<Control>l"]);
        self.inner
            .set_accels_for_action("win.reload", &["F5", "<Control>r"]);
    }
}

fn handle(
    application: &adw::Application,
    window: &OnceCell<Rc<MainWindow>>,
    shared: &Shared<'_>,
    request: &LaunchRequest,
    command_line: &gio::ApplicationCommandLine,
) {
    for problem in &request.problems {
        command_line.printerr_literal(&format!("wazzup: {problem}\n"));
    }
    if request.quit {
        match window.get() {
            Some(main_window) => main_window.quit(),
            None => application.quit(),
        }
        return;
    }
    let first_launch = window.get().is_none();
    let main_window = window.get_or_init(|| MainWindow::new(application, shared));
    let stay_hidden = request.in_background
        || (first_launch
            && shared.store.snapshot().start_in_background
            && request.chats.is_empty());
    let chats: Vec<ChatLink> = request
        .chats
        .iter()
        .filter_map(|target| {
            let parsed = ChatLink::parse(target);
            if parsed.is_none() {
                command_line.printerr_literal(&format!("wazzup: not a chat link: {target}\n"));
            }
            parsed
        })
        .collect();
    if let Some(chat) = chats.last() {
        main_window.open_chat(&chat.address());
    } else if !stay_hidden {
        main_window.present();
    }
    if request.preferences {
        main_window.present();
        main_window.show_preferences();
    }
}

fn apply_theme(preferences: &Preferences) {
    let scheme = match preferences.theme {
        ThemeChoice::System => adw::ColorScheme::Default,
        ThemeChoice::Light => adw::ColorScheme::ForceLight,
        ThemeChoice::Dark => adw::ColorScheme::ForceDark,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

fn sync_tray(tray: &RefCell<TrayIcon>, preferences: &Preferences) {
    let mut tray = tray.borrow_mut();
    tray.set_style(preferences.tray_style);
    tray.set_visible(preferences.tray_icon);
}

fn sync_autostart(enabled: bool) {
    if let Err(error) = XdgAutostart::for_this_installation().apply(enabled) {
        tracing::warn!(%error, "cannot update launch at login");
    }
}
