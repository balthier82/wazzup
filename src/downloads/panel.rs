use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use super::history::{DownloadRecord, History, is_risky, readable_size};
use crate::config::{APP_ID, Profile};
use crate::engine::{DownloadControl, DownloadProgress, DownloadState};
use crate::i18n::{t, t_with};

struct Entry {
    row: gtk::ListBoxRow,
    status: gtk::Label,
    progress: gtk::ProgressBar,
    cancel: gtk::Button,
    open: gtk::Button,
}

pub struct DownloadsPanel {
    button: gtk::MenuButton,
    list: gtk::ListBox,
    history: RefCell<History>,
    entries: RefCell<HashMap<PathBuf, Entry>>,
    controls: Rc<RefCell<HashMap<PathBuf, DownloadControl>>>,
    location: PathBuf,
}

impl DownloadsPanel {
    pub fn new() -> Rc<Self> {
        let location = glib::user_data_dir().join(APP_ID).join(format!(
            "{}-downloads.toml",
            Profile::from_environment().engine_folder()
        ));
        let history =
            std::fs::read_to_string(&location).map_or_default(|source| History::from_toml(&source));
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let clear = gtk::Button::builder()
            .label(t("Clear list"))
            .halign(gtk::Align::End)
            .css_classes(["flat"])
            .build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .width_request(360)
            .build();
        content.append(
            &gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .propagate_natural_height(true)
                .max_content_height(420)
                .child(&list)
                .build(),
        );
        content.append(&clear);
        let button = gtk::MenuButton::builder()
            .icon_name("folder-download-symbolic")
            .tooltip_text(t("Downloads"))
            .popover(&gtk::Popover::builder().child(&content).build())
            .build();
        let panel = Rc::new(Self {
            button,
            list,
            history: RefCell::new(history),
            entries: RefCell::default(),
            controls: Rc::default(),
            location,
        });
        let weak = Rc::downgrade(&panel);
        clear.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.clear();
            }
        });
        panel.rebuild();
        panel
    }

    pub fn button(&self) -> &gtk::MenuButton {
        &self.button
    }

    pub fn track(&self, progress: &DownloadProgress, control: Option<DownloadControl>) {
        if let Some(control) = control.filter(|_| progress.state == DownloadState::Running) {
            self.controls
                .borrow_mut()
                .insert(progress.path.clone(), control);
        }
        let record = DownloadRecord {
            path: progress.path.clone(),
            received: progress.received,
            total: progress.total,
            state: progress.state,
        };
        let previous = self
            .history
            .borrow()
            .records
            .iter()
            .find(|known| known.path == record.path)
            .map(|known| known.state);
        let is_new = previous.is_none();
        let state_changed = previous != Some(record.state);
        if record.state != DownloadState::Running {
            self.controls.borrow_mut().remove(&record.path);
        }
        self.history.borrow_mut().remember(record.clone());
        if is_new {
            self.rebuild();
            self.button.popup();
        } else if let Some(entry) = self.entries.borrow().get(&record.path) {
            refresh(entry, &record);
        }
        if state_changed {
            self.persist();
        }
        if previous.is_some() && state_changed && record.state == DownloadState::Finished {
            self.announce(&record);
        }
    }

    fn announce(&self, record: &DownloadRecord) {
        let Some(application) = self
            .button
            .root()
            .and_downcast::<gtk::Window>()
            .and_then(|window| window.application())
        else {
            return;
        };
        let notification = gio::Notification::new(&t("Download complete"));
        notification.set_body(Some(&record.file_name()));
        notification.set_default_action("app.present");
        application.send_notification(None, &notification);
    }

    fn clear(&self) {
        self.history.borrow_mut().forget_finished();
        self.persist();
        self.rebuild();
    }

    fn rebuild(&self) {
        self.list.remove_all();
        self.entries.borrow_mut().clear();
        let records = self.history.borrow().records.clone();
        self.button.set_visible(!records.is_empty());
        for record in records {
            let entry = self.entry_for(&record);
            refresh(&entry, &record);
            self.list.append(&entry.row);
            self.entries.borrow_mut().insert(record.path.clone(), entry);
        }
    }

    fn entry_for(&self, record: &DownloadRecord) -> Entry {
        let name = gtk::Label::builder()
            .label(record.file_name())
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        let status = gtk::Label::builder()
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build();
        let progress = gtk::ProgressBar::new();
        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .build();
        text.append(&name);
        text.append(&progress);
        text.append(&status);
        let cancel = icon_button("process-stop-symbolic", &t("Cancel"));
        let open = icon_button("document-open-symbolic", &t("Open"));
        let folder = icon_button("folder-open-symbolic", &t("Show in folder"));
        let controls = Rc::clone(&self.controls);
        let path = record.path.clone();
        cancel.connect_clicked(move |_| {
            if let Some(control) = controls.borrow().get(&path) {
                control.cancel();
            }
        });
        let path = record.path.clone();
        open.connect_clicked(move |button| open_file(button, &path));
        let path = record.path.clone();
        folder.connect_clicked(move |button| reveal_file(button, &path));
        let line = gtk::Box::builder()
            .spacing(6)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(6)
            .build();
        line.append(&text);
        line.append(&cancel);
        line.append(&open);
        line.append(&folder);
        let row = gtk::ListBoxRow::builder()
            .activatable(false)
            .child(&line)
            .build();
        Entry {
            row,
            status,
            progress,
            cancel,
            open,
        }
    }

    fn persist(&self) {
        let written = self
            .location
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&self.location, self.history.borrow().to_toml()));
        if let Err(error) = written {
            tracing::warn!(%error, "cannot save download history");
        }
    }
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .css_classes(["flat", "circular"])
        .build()
}

fn refresh(entry: &Entry, record: &DownloadRecord) {
    let running = record.state == DownloadState::Running;
    entry.progress.set_visible(running);
    entry.cancel.set_visible(running);
    entry
        .open
        .set_visible(record.state == DownloadState::Finished);
    let status = match record.state {
        DownloadState::Running => {
            if let Some(total) = record.total {
                #[allow(clippy::cast_precision_loss)]
                entry
                    .progress
                    .set_fraction(record.received as f64 / total.max(1) as f64);
                t_with(
                    "{done} of {total}",
                    &[
                        ("done", &readable_size(record.received)),
                        ("total", &readable_size(total)),
                    ],
                )
            } else {
                entry.progress.pulse();
                readable_size(record.received)
            }
        }
        DownloadState::Finished => readable_size(record.received),
        DownloadState::Cancelled => t("Cancelled"),
        DownloadState::Failed => t("Failed"),
    };
    entry.status.set_label(&status);
}

fn parent_window(widget: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    widget.root().and_downcast::<gtk::Window>()
}

fn open_file(button: &gtk::Button, path: &Path) {
    let parent = parent_window(button);
    let launch = {
        let path = path.to_path_buf();
        let parent = parent.clone();
        move || {
            gtk::FileLauncher::new(Some(&gio::File::for_path(&path))).launch(
                parent.as_ref(),
                None::<&gio::Cancellable>,
                |outcome| {
                    if let Err(error) = outcome {
                        tracing::warn!(%error, "cannot open download");
                    }
                },
            );
        }
    };
    if !is_risky(path) {
        launch();
        return;
    }
    let dialog = adw::AlertDialog::builder()
        .heading(t("Open this file?"))
        .body(t(
            "It can run programs on your computer. Open it only if you trust who sent it.",
        ))
        .close_response("cancel")
        .default_response("cancel")
        .build();
    dialog.add_responses(&[("cancel", &t("Cancel")), ("open", &t("Open Anyway"))]);
    dialog.set_response_appearance("open", adw::ResponseAppearance::Destructive);
    dialog.connect_response(Some("open"), move |_, _| launch());
    dialog.present(parent.as_ref());
}

fn reveal_file(button: &gtk::Button, path: &Path) {
    gtk::FileLauncher::new(Some(&gio::File::for_path(path))).open_containing_folder(
        parent_window(button).as_ref(),
        None::<&gio::Cancellable>,
        |outcome| {
            if let Err(error) = outcome {
                tracing::warn!(%error, "cannot show download folder");
            }
        },
    );
}
