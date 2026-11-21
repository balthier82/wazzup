use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;

use crate::browser::{BrowserSurface, SurfaceObserver};
use crate::config::APP_NAME;

pub struct CallWindow {
    window: adw::Window,
    surface: BrowserSurface,
}

impl CallWindow {
    pub fn open(application: &adw::Application, width: i32, height: i32) -> BrowserSurface {
        let call = Rc::new_cyclic(|this: &Weak<Self>| {
            let observer: Weak<dyn SurfaceObserver> = this.clone();
            let surface = BrowserSurface::new(observer);
            let toolbar = adw::ToolbarView::new();
            toolbar.add_top_bar(&adw::HeaderBar::new());
            toolbar.set_content(Some(surface.widget()));
            let window = adw::Window::builder()
                .application(application)
                .title(APP_NAME)
                .default_width(width.max(360))
                .default_height(height.max(360))
                .content(&toolbar)
                .build();
            Self { window, surface }
        });
        let keeper = Rc::clone(&call);
        call.window.connect_close_request(move |_| {
            if keeper.surface.request_close() {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        call.window.present();
        call.surface.clone()
    }
}

impl SurfaceObserver for CallWindow {
    fn title_changed(&self, title: &str) {
        let shown = if title.trim().is_empty() {
            APP_NAME
        } else {
            title
        };
        self.window.set_title(Some(shown));
    }

    fn surface_closed(&self) {
        self.window.destroy();
    }
}
