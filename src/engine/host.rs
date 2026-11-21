use std::rc::Rc;

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
}
