use cef::{
    CefString, EventFlags, ImplMenuModel, ImplRunContextMenuCallback, MenuItemType, MenuModel,
    RunContextMenuCallback,
};

const HIDDEN_COMMANDS: [i32; 7] = [100, 101, 102, 103, 104, 131, 132];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Command {
        command: i32,
        label: String,
        enabled: bool,
    },
    Separator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRequest {
    pub x: i32,
    pub y: i32,
    pub entries: Vec<MenuEntry>,
}

pub struct MenuAnswer(RunContextMenuCallback);

impl MenuAnswer {
    pub fn new(callback: RunContextMenuCallback) -> Self {
        Self(callback)
    }

    pub fn choose(self, command: i32) {
        self.0.cont(command, EventFlags::default());
    }

    pub fn dismiss(self) {
        self.0.cancel();
    }
}

pub fn trim_model(model: &MenuModel) {
    for command in HIDDEN_COMMANDS {
        model.remove(command);
    }
}

pub fn read_model(model: &MenuModel) -> Vec<MenuEntry> {
    let entries = (0..model.count()).filter_map(|index| {
        let kind = model.type_at(index);
        if kind == MenuItemType::SEPARATOR {
            return Some(MenuEntry::Separator);
        }
        if kind != MenuItemType::COMMAND && kind != MenuItemType::CHECK {
            return None;
        }
        Some(MenuEntry::Command {
            command: model.command_id_at(index),
            label: clean_label(&CefString::from(&model.label_at(index)).to_string()),
            enabled: model.is_enabled_at(index) == 1,
        })
    });
    tidy(entries.collect())
}

fn tidy(entries: Vec<MenuEntry>) -> Vec<MenuEntry> {
    let mut tidied: Vec<MenuEntry> = Vec::with_capacity(entries.len());
    for entry in entries {
        let redundant = entry == MenuEntry::Separator
            && tidied
                .last()
                .is_none_or(|last| *last == MenuEntry::Separator);
        if !redundant {
            tidied.push(entry);
        }
    }
    if tidied.last() == Some(&MenuEntry::Separator) {
        tidied.pop();
    }
    tidied
}

pub fn clean_label(raw: &str) -> String {
    let mut cleaned = String::with_capacity(raw.len());
    let mut characters = raw.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '&' {
            if characters.peek() == Some(&'&') {
                cleaned.push('&');
                characters.next();
            }
            continue;
        }
        cleaned.push(character);
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::{MenuEntry, clean_label, tidy};

    fn command(label: &str) -> MenuEntry {
        MenuEntry::Command {
            command: 1,
            label: label.to_owned(),
            enabled: true,
        }
    }

    #[test]
    fn removes_mnemonic_markers() {
        assert_eq!(clean_label("&Copy"), "Copy");
        assert_eq!(clean_label("Save && Close"), "Save & Close");
    }

    #[test]
    fn collapses_separators() {
        let tidied = tidy(vec![
            MenuEntry::Separator,
            command("Copy"),
            MenuEntry::Separator,
            MenuEntry::Separator,
            command("Paste"),
            MenuEntry::Separator,
        ]);
        assert_eq!(
            tidied,
            vec![command("Copy"), MenuEntry::Separator, command("Paste")]
        );
    }
}
