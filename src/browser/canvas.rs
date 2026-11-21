use std::cell::RefCell;

use gtk::gdk;
use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::{RefCell, gdk, glib, graphene};
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct BrowserCanvas {
        pub page: RefCell<Option<(gdk::Texture, f64)>>,
        pub popup_texture: RefCell<Option<gdk::Texture>>,
        pub popup_area: RefCell<Option<graphene::Rect>>,
        pub resized: RefCell<Option<Box<dyn Fn()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrowserCanvas {
        const NAME: &'static str = "WazzupBrowserCanvas";
        type Type = super::BrowserCanvas;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for BrowserCanvas {
        fn constructed(&self) {
            self.parent_constructed();
            let canvas = self.obj();
            canvas.set_focusable(true);
            canvas.set_hexpand(true);
            canvas.set_vexpand(true);
            canvas.set_overflow(gtk::Overflow::Hidden);
        }
    }

    impl WidgetImpl for BrowserCanvas {
        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            self.notify_resized();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some((texture, scale)) = self.page.borrow().as_ref() {
                snapshot.append_texture(texture, &logical_bounds(texture, *scale));
            }
            let popup_texture = self.popup_texture.borrow();
            let popup_area = self.popup_area.borrow();
            if let (Some(texture), Some(area)) = (popup_texture.as_ref(), popup_area.as_ref()) {
                snapshot.append_texture(texture, area);
            }
        }
    }

    impl BrowserCanvas {
        pub fn notify_resized(&self) {
            if let Some(callback) = self.resized.borrow().as_ref() {
                callback();
            }
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn logical_bounds(texture: &gdk::Texture, scale: f64) -> graphene::Rect {
        let divisor = if scale > 0.0 { scale } else { 1.0 };
        graphene::Rect::new(
            0.0,
            0.0,
            (f64::from(texture.width()) / divisor) as f32,
            (f64::from(texture.height()) / divisor) as f32,
        )
    }
}

glib::wrapper! {
    pub struct BrowserCanvas(ObjectSubclass<imp::BrowserCanvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for BrowserCanvas {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl BrowserCanvas {
    pub fn device_scale(&self) -> f64 {
        self.native()
            .and_then(|native| native.surface())
            .map_or_else(|| f64::from(self.scale_factor()), |surface| surface.scale())
    }

    pub fn connect_resized<F: Fn() + 'static>(&self, callback: F) {
        self.imp().resized.replace(Some(Box::new(callback)));
        self.connect_scale_factor_notify(|canvas| canvas.imp().notify_resized());
    }

    pub fn show_page(&self, texture: gdk::Texture, scale: f64) {
        self.imp().page.replace(Some((texture, scale)));
        self.queue_draw();
    }

    pub fn show_popup_frame(&self, texture: gdk::Texture) {
        self.imp().popup_texture.replace(Some(texture));
        self.queue_draw();
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn place_popup(&self, area: Option<(i32, i32, i32, i32)>) {
        let imp = self.imp();
        if let Some((x, y, width, height)) = area {
            imp.popup_area.replace(Some(graphene::Rect::new(
                x as f32,
                y as f32,
                width as f32,
                height as f32,
            )));
        } else {
            imp.popup_area.replace(None);
            imp.popup_texture.replace(None);
        }
        self.queue_draw();
    }
}
