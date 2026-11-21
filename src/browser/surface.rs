use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use cef::{
    Browser, BrowserHost, BrowserSettings, CefString, ImplBrowser, ImplBrowserHost, ImplFrame,
    RuntimeStyle, WindowInfo, browser_host_create_browser,
};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use super::canvas::BrowserCanvas;
use super::input;
use crate::engine::{
    DownloadControl, DownloadProgress, FileAnswer, FileRequest, MediaAccess, PageClient, PageHost,
    PageMessage, PaintLayer, PixelFrame, PopupArea, Viewport, emulate_color_scheme,
};

const FRAME_RATE: i32 = 60;

pub trait SurfaceObserver {
    fn title_changed(&self, _title: &str) {}
    fn notification_posted(&self, _id: u64, _title: &str, _body: &str) {}
    fn notification_withdrawn(&self, _id: u64) {}
    fn surface_closed(&self) {}
    fn companion_requested(&self, _width: i32, _height: i32) -> Option<BrowserSurface> {
        None
    }
    fn page_loaded(&self) {}
    fn media_allowed(&self) -> MediaAccess {
        MediaAccess::default()
    }
    fn download_target(&self, _suggested_name: &str) -> Option<PathBuf> {
        None
    }
    fn download_progress(&self, _progress: DownloadProgress, _control: Option<DownloadControl>) {}
    fn files_allowed(&self) -> bool {
        true
    }
}

pub struct SurfaceCore {
    canvas: BrowserCanvas,
    im: gtk::IMMulticontext,
    browser: RefCell<Option<Browser>>,
    viewport: Cell<Viewport>,
    pending_address: RefCell<Option<String>>,
    observer: Weak<dyn SurfaceObserver>,
}

impl SurfaceCore {
    pub fn host(&self) -> Option<BrowserHost> {
        self.browser.borrow().as_ref().and_then(ImplBrowser::host)
    }

    fn refresh_viewport(&self) {
        let next = Viewport {
            width: self.canvas.width().max(1),
            height: self.canvas.height().max(1),
            scale: self.canvas.device_scale(),
        };
        let previous = self.viewport.replace(next);
        let Some(host) = self.host() else {
            return;
        };
        if (previous.scale - next.scale).abs() > f64::EPSILON {
            host.notify_screen_info_changed();
        }
        if previous != next {
            host.was_resized();
        }
    }

    fn follow_style(&self) {
        if let Some(host) = self.host() {
            emulate_color_scheme(&host, adw::StyleManager::default().is_dark());
        }
    }

    fn navigate(&self, address: &str) {
        let frame = self
            .browser
            .borrow()
            .as_ref()
            .and_then(ImplBrowser::main_frame);
        match frame {
            Some(frame) => frame.load_url(Some(&CefString::from(address))),
            None => {
                self.pending_address.replace(Some(address.to_owned()));
            }
        }
    }

    fn observer(&self) -> Option<Rc<dyn SurfaceObserver>> {
        self.observer.upgrade()
    }
}

impl PageHost for SurfaceCore {
    fn viewport(&self) -> Viewport {
        self.viewport.get()
    }

    fn paint(&self, layer: PaintLayer, frame: &PixelFrame<'_>) {
        let Ok(stride) = usize::try_from(frame.width).map(|width| width * 4) else {
            return;
        };
        let texture = gdk::MemoryTexture::new(
            frame.width,
            frame.height,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &glib::Bytes::from(frame.pixels),
            stride,
        )
        .upcast::<gdk::Texture>();
        match layer {
            PaintLayer::Page => self.canvas.show_page(texture, self.viewport.get().scale),
            PaintLayer::Popup => self.canvas.show_popup_frame(texture),
        }
    }

    fn show_popup(&self, area: Option<PopupArea>) {
        self.canvas
            .place_popup(area.map(|area| (area.x, area.y, area.width, area.height)));
    }

    fn set_cursor(&self, name: &'static str) {
        self.canvas.set_cursor_from_name(Some(name));
    }

    fn set_caret(&self, area: PopupArea) {
        self.im.set_cursor_location(&gdk::Rectangle::new(
            area.x,
            area.y,
            area.width,
            area.height,
        ));
    }

    fn attach(&self, browser: Browser) {
        self.browser.replace(Some(browser));
        self.refresh_viewport();
        if let Some(host) = self.host() {
            host.was_resized();
            host.set_focus(i32::from(self.canvas.has_focus()));
        }
        self.follow_style();
        if let Some(address) = self.pending_address.take() {
            self.navigate(&address);
        }
    }

    fn detach(&self) {
        self.browser.replace(None);
        if let Some(observer) = self.observer() {
            observer.surface_closed();
        }
    }

    fn title_changed(&self, title: &str) {
        if let Some(observer) = self.observer() {
            observer.title_changed(title);
        }
    }

    fn page_message(&self, message: PageMessage) {
        let Some(observer) = self.observer() else {
            return;
        };
        match message {
            PageMessage::Notification { id, title, body } => {
                observer.notification_posted(id, &title, &body);
            }
            PageMessage::NotificationClosed { id } => observer.notification_withdrawn(id),
        }
    }

    fn open_externally(&self, address: &str) {
        tracing::info!(%address, "opening in system browser");
        let parent = self.canvas.root().and_downcast::<gtk::Window>();
        gtk::UriLauncher::new(address).launch(
            parent.as_ref(),
            None::<&gtk::gio::Cancellable>,
            |outcome| {
                if let Err(error) = outcome {
                    tracing::warn!(%error, "cannot open link");
                }
            },
        );
    }

    fn page_loaded(&self) {
        if let Some(observer) = self.observer() {
            observer.page_loaded();
        }
    }

    fn media_allowed(&self) -> MediaAccess {
        self.observer()
            .map_or_else(MediaAccess::default, |observer| observer.media_allowed())
    }

    fn download_target(&self, suggested_name: &str) -> Option<PathBuf> {
        self.observer()?.download_target(suggested_name)
    }

    fn download_progress(&self, progress: DownloadProgress, control: Option<DownloadControl>) {
        if let Some(observer) = self.observer() {
            observer.download_progress(progress, control);
        }
    }

    fn choose_files(&self, request: FileRequest, answer: FileAnswer) {
        if !self
            .observer()
            .is_some_and(|observer| observer.files_allowed())
        {
            answer.dismiss();
            return;
        }
        let parent = self.canvas.root().and_downcast::<gtk::Window>();
        super::file_picker::present(parent.as_ref(), &request, answer);
    }

    fn open_companion(&self, width: i32, height: i32) -> Option<Rc<dyn PageHost>> {
        let surface = self.observer()?.companion_requested(width, height)?;
        Some(surface.core as Rc<dyn PageHost>)
    }
}

#[derive(Clone)]
pub struct BrowserSurface {
    core: Rc<SurfaceCore>,
}

impl BrowserSurface {
    pub fn new(observer: Weak<dyn SurfaceObserver>) -> Self {
        let canvas = BrowserCanvas::default();
        let im = gtk::IMMulticontext::new();
        let core = Rc::new(SurfaceCore {
            canvas: canvas.clone(),
            im: im.clone(),
            browser: RefCell::default(),
            viewport: Cell::default(),
            pending_address: RefCell::default(),
            observer,
        });
        input::attach(&canvas, Rc::downgrade(&core), &im);
        let styled = Rc::downgrade(&core);
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(core) = styled.upgrade() {
                core.follow_style();
            }
        });
        let resized = Rc::downgrade(&core);
        canvas.connect_resized(move || {
            if let Some(core) = resized.upgrade() {
                core.refresh_viewport();
            }
        });
        Self { core }
    }

    pub fn widget(&self) -> &BrowserCanvas {
        &self.core.canvas
    }

    pub fn load(&self, address: &str) {
        self.core.refresh_viewport();
        let window_info = WindowInfo {
            windowless_rendering_enabled: 1,
            runtime_style: RuntimeStyle::ALLOY,
            ..Default::default()
        };
        let settings = BrowserSettings {
            windowless_frame_rate: FRAME_RATE,
            ..Default::default()
        };
        let mut client = PageClient::new(Rc::clone(&self.core) as Rc<dyn PageHost>);
        let created = browser_host_create_browser(
            Some(&window_info),
            Some(&mut client),
            Some(&CefString::from(address)),
            Some(&settings),
            None,
            None,
        );
        if created != 1 {
            tracing::error!(%address, "browser could not be created");
        }
    }

    pub fn navigate(&self, address: &str) {
        self.core.navigate(address);
    }

    pub fn reload(&self) {
        if let Some(browser) = self.core.browser.borrow().as_ref() {
            browser.reload();
        }
    }

    pub fn request_close(&self) -> bool {
        match self.core.host() {
            Some(host) => {
                host.close_browser(0);
                true
            }
            None => false,
        }
    }

    pub fn apply_veil(&self, pixels: u32) {
        self.run_script(&PageMessage::veil_script(pixels));
    }

    pub fn activate_notification(&self, id: u64) {
        self.run_script(&PageMessage::activation_script(id));
    }

    fn run_script(&self, code: &str) {
        let frame = self
            .core
            .browser
            .borrow()
            .as_ref()
            .and_then(ImplBrowser::main_frame);
        if let Some(frame) = frame {
            frame.execute_java_script(Some(&CefString::from(code)), None, 0);
        }
    }
}
