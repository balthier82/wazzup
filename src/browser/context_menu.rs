use std::cell::RefCell;
use std::rc::Rc;

use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use crate::engine::{MenuAnswer, MenuEntry, MenuRequest};

const GROUP: &str = "page";

pub fn present(anchor: &impl IsA<gtk::Widget>, request: &MenuRequest, answer: MenuAnswer) {
    let answer = Rc::new(RefCell::new(Some(answer)));
    let actions = gio::SimpleActionGroup::new();
    let menu = gio::Menu::new();
    let mut section = gio::Menu::new();
    for (index, entry) in request.entries.iter().enumerate() {
        match entry {
            MenuEntry::Separator => {
                menu.append_section(None, &section);
                section = gio::Menu::new();
            }
            MenuEntry::Command {
                command,
                label,
                enabled,
            } => {
                let name = format!("entry{index}");
                let action = gio::SimpleAction::new(&name, None);
                action.set_enabled(*enabled);
                let (answer, command) = (Rc::clone(&answer), *command);
                action.connect_activate(move |_, _| {
                    if let Some(answer) = answer.borrow_mut().take() {
                        answer.choose(command);
                    }
                });
                actions.add_action(&action);
                section.append(Some(label), Some(&format!("{GROUP}.{name}")));
            }
        }
    }
    menu.append_section(None, &section);

    let popover = gtk::PopoverMenu::builder()
        .menu_model(&menu)
        .has_arrow(false)
        .halign(gtk::Align::Start)
        .pointing_to(&gdk::Rectangle::new(request.x, request.y, 1, 1))
        .build();
    popover.insert_action_group(GROUP, Some(&actions));
    popover.set_parent(anchor);
    popover.connect_closed(move |popover| {
        let answer = Rc::clone(&answer);
        let popover = popover.clone();
        glib::idle_add_local_once(move || {
            if let Some(answer) = answer.borrow_mut().take() {
                answer.dismiss();
            }
            popover.unparent();
        });
    });
    popover.popup();
}
