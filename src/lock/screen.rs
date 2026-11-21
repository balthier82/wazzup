use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

use super::{AppLock, UnlockOutcome};
use crate::i18n::{t, t_with};

#[derive(Clone)]
pub struct LockScreen {
    root: adw::ToolbarView,
    entry: adw::PasswordEntryRow,
    message: gtk::Label,
}

impl LockScreen {
    pub fn new(lock: &Rc<AppLock>) -> Self {
        let entry = adw::PasswordEntryRow::builder()
            .title(t("Passcode"))
            .build();
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list.append(&entry);
        let unlock = gtk::Button::builder()
            .label(t("Unlock"))
            .halign(gtk::Align::Center)
            .css_classes(["suggested-action", "pill"])
            .build();
        let message = gtk::Label::builder()
            .wrap(true)
            .justify(gtk::Justification::Center)
            .css_classes(["error"])
            .visible(false)
            .build();
        let form = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .build();
        form.append(&list);
        form.append(&message);
        form.append(&unlock);
        let clamp = adw::Clamp::builder().maximum_size(360).child(&form).build();
        let status = adw::StatusPage::builder()
            .icon_name("system-lock-screen-symbolic")
            .title(t("Wazzup is locked"))
            .description(t("Enter your passcode to continue"))
            .child(&clamp)
            .vexpand(true)
            .build();
        let root = adw::ToolbarView::new();
        root.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
        root.set_content(Some(&status));

        let screen = Self {
            root,
            entry,
            message,
        };
        screen.wire(lock, &unlock);
        screen
    }

    pub fn widget(&self) -> &adw::ToolbarView {
        &self.root
    }

    pub fn reset(&self) {
        self.entry.set_text("");
        self.message.set_visible(false);
        self.entry.grab_focus();
    }

    fn wire(&self, lock: &Rc<AppLock>, unlock: &gtk::Button) {
        let attempt = {
            let lock = Rc::clone(lock);
            let screen = self.clone();
            move || screen.attempt(&lock)
        };
        let from_entry = attempt.clone();
        self.entry.connect_entry_activated(move |_| from_entry());
        unlock.connect_clicked(move |_| attempt());
    }

    fn attempt(&self, lock: &AppLock) {
        let secret = self.entry.text();
        if secret.is_empty() {
            return;
        }
        match lock.try_unlock(&secret) {
            UnlockOutcome::Unlocked => self.reset(),
            UnlockOutcome::Rejected { wait } => {
                self.entry.set_text("");
                self.explain(&t("Wrong passcode"), wait);
            }
            UnlockOutcome::TooSoon { wait } => self.explain(&t("Too many attempts"), wait),
        }
    }

    fn explain(&self, reason: &str, wait: Duration) {
        self.message.set_visible(true);
        if wait.is_zero() {
            self.message.set_label(reason);
            return;
        }
        self.message.set_label(&t_with(
            "{reason}. Try again in {seconds} s.",
            &[("reason", &reason), ("seconds", &wait.as_secs())],
        ));
        self.entry.set_sensitive(false);
        let screen = self.clone();
        glib::timeout_add_local_once(wait, move || {
            screen.entry.set_sensitive(true);
            screen.message.set_visible(false);
            screen.entry.grab_focus();
        });
    }
}
