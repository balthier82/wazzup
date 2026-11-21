use std::path::PathBuf;
use std::rc::Rc;

use super::context_menu::{MenuAnswer, MenuRequest};
use super::download::{DownloadControl, DownloadProgress};
use super::file_dialog::{FileAnswer, FileRequest};
use super::media::MediaAccess;
use super::page_message::PageMessage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
            scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintLayer {
    Page,
    Popup,
}

pub struct PixelFrame<'a> {
    pub pixels: &'a [u8],
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub trait PageHost {
    fn viewport(&self) -> Viewport;
    fn paint(&self, layer: PaintLayer, frame: &PixelFrame<'_>);
    fn show_popup(&self, area: Option<PopupArea>);
    fn set_cursor(&self, name: &'static str);
    fn set_caret(&self, area: PopupArea);
    fn attach(&self, browser: cef::Browser);
    fn detach(&self);
    fn title_changed(&self, title: &str);
    fn page_message(&self, message: PageMessage);
    fn open_externally(&self, address: &str);
    fn open_companion(&self, width: i32, height: i32) -> Option<Rc<dyn PageHost>>;
    fn page_loaded(&self);
    fn media_allowed(&self) -> MediaAccess;
    fn download_target(&self, suggested_name: &str) -> Option<PathBuf>;
    fn download_progress(&self, progress: DownloadProgress, control: Option<DownloadControl>);
    fn choose_files(&self, request: FileRequest, answer: FileAnswer);
    fn page_crashed(&self);
    fn proxy_credentials(&self) -> Option<(String, String)>;
    fn show_context_menu(&self, request: MenuRequest, answer: MenuAnswer);
    fn load_failed(&self, description: &str);
}
