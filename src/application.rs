use std::cell::OnceCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::config::Profile;
use crate::window::MainWindow;

pub struct WazzupApplication {
    inner: adw::Application,
    window: Rc<OnceCell<Rc<MainWindow>>>,
}

impl WazzupApplication {
    pub fn new() -> Self {
        let inner = adw::Application::builder()
            .application_id(Profile::from_environment().application_id())
            .flags(gio::ApplicationFlags::empty())
            .build();
        let application = Self {
            inner,
            window: Rc::default(),
        };
        application.wire();
        application
    }

    pub fn run(&self) -> glib::ExitCode {
        self.inner.run_with_args::<&str>(&[])
    }

    fn wire(&self) {
        let window = Rc::clone(&self.window);
        self.inner.connect_activate(move |application| {
            window
                .get_or_init(|| MainWindow::new(application))
                .present();
        });

        let window = Rc::clone(&self.window);
        let present = gio::ActionEntry::builder("present")
            .activate(
                move |application: &adw::Application, _, _| match window.get() {
                    Some(main_window) => main_window.present(),
                    None => application.activate(),
                },
            )
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

        let quit = gio::ActionEntry::builder("quit")
            .activate(|application: &adw::Application, _, _| {
                for window in application.windows() {
                    window.close();
                }
            })
            .build();

        self.inner
            .add_action_entries([present, open_notification, quit]);
        self.inner
            .set_accels_for_action("app.quit", &["<Control>q"]);
        self.inner
            .set_accels_for_action("win.reload", &["F5", "<Control>r"]);
    }
}
