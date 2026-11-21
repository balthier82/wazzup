mod glyph;

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, ToolTip, Tray};

use crate::config::{APP_ID, APP_NAME};
use crate::i18n::{t, t_count};
use crate::preferences::TrayStyle;
use glyph::{Glyph, Ink};

const ICON_SIZES: [i32; 3] = [22, 32, 48];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    ToggleWindow,
    OpenWindow,
    OpenPreferences,
    Quit,
}

struct TrayModel {
    style: TrayStyle,
    unread: u32,
    commands: async_channel::Sender<TrayCommand>,
}

impl TrayModel {
    fn send(&self, command: TrayCommand) {
        if self.commands.send_blocking(command).is_err() {
            tracing::debug!(?command, "tray command dropped");
        }
    }

    fn ink(&self) -> Ink {
        match self.style {
            TrayStyle::Light => Ink::LIGHT,
            TrayStyle::Dark => Ink::DARK,
        }
    }
}

impl Tray for TrayModel {
    fn id(&self) -> String {
        APP_ID.to_owned()
    }

    fn title(&self) -> String {
        APP_NAME.to_owned()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        ICON_SIZES
            .iter()
            .filter_map(|size| Glyph::draw(*size, self.ink(), self.unread > 0))
            .map(|glyph| Icon {
                width: glyph.size,
                height: glyph.size,
                data: glyph.argb,
            })
            .collect()
    }

    fn tool_tip(&self) -> ToolTip {
        let description = if self.unread == 0 {
            t("No unread chats")
        } else {
            t_count(
                "{count} unread chat",
                "{count} unread chats",
                u64::from(self.unread),
            )
        };
        ToolTip {
            title: APP_NAME.to_owned(),
            description,
            ..ToolTip::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::ToggleWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let entry = |label: &str, command: TrayCommand| -> MenuItem<Self> {
            StandardItem {
                label: t(label),
                activate: Box::new(move |model: &mut Self| model.send(command)),
                ..StandardItem::default()
            }
            .into()
        };
        vec![
            entry("Open Wazzup", TrayCommand::OpenWindow),
            entry("Preferences", TrayCommand::OpenPreferences),
            MenuItem::Separator,
            entry("Quit", TrayCommand::Quit),
        ]
    }
}

pub struct TrayIcon {
    handle: Option<Handle<TrayModel>>,
    commands: async_channel::Sender<TrayCommand>,
    style: TrayStyle,
    unread: u32,
}

impl TrayIcon {
    pub fn new() -> (Self, async_channel::Receiver<TrayCommand>) {
        let (commands, receiver) = async_channel::unbounded();
        let icon = Self {
            handle: None,
            commands,
            style: TrayStyle::default(),
            unread: 0,
        };
        (icon, receiver)
    }

    pub fn set_visible(&mut self, visible: bool) {
        match (visible, self.handle.is_some()) {
            (true, false) => self.spawn(),
            (false, true) => {
                if let Some(handle) = self.handle.take() {
                    handle.shutdown().wait();
                }
            }
            _ => {}
        }
    }

    pub fn set_style(&mut self, style: TrayStyle) {
        self.style = style;
        self.refresh();
    }

    pub fn set_unread(&mut self, unread: u32) {
        if self.unread != unread {
            self.unread = unread;
            self.refresh();
        }
    }

    fn spawn(&mut self) {
        let model = TrayModel {
            style: self.style,
            unread: self.unread,
            commands: self.commands.clone(),
        };
        match model.spawn() {
            Ok(handle) => self.handle = Some(handle),
            Err(error) => tracing::warn!(%error, "top bar icon unavailable"),
        }
    }

    fn refresh(&self) {
        let (style, unread) = (self.style, self.unread);
        if let Some(handle) = &self.handle {
            handle.update(|model| {
                model.style = style;
                model.unread = unread;
            });
        }
    }
}
