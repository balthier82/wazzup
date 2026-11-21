use std::path::PathBuf;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use crate::engine::{FileAnswer, FileChoice, FileRequest};

pub fn present(parent: Option<&gtk::Window>, request: &FileRequest, answer: FileAnswer) {
    let dialog = gtk::FileDialog::builder().modal(true).build();
    if !request.title.is_empty() {
        dialog.set_title(&request.title);
    }
    if let Some(filter) = filter_for(&request.accepted) {
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        dialog.set_default_filter(Some(&filter));
    }
    if request.choice == FileChoice::Save {
        suggest_destination(&dialog, request);
    }
    let cancellable = None::<&gio::Cancellable>;
    match request.choice {
        FileChoice::OpenOne => dialog.open(parent, cancellable, move |outcome| {
            answer.choose(&single(outcome));
        }),
        FileChoice::OpenMany => dialog.open_multiple(parent, cancellable, move |outcome| {
            answer.choose(&outcome.map_or_default(|files| paths_of(&files)));
        }),
        FileChoice::OpenFolder => dialog.select_folder(parent, cancellable, move |outcome| {
            answer.choose(&single(outcome));
        }),
        FileChoice::Save => dialog.save(parent, cancellable, move |outcome| {
            answer.choose(&single(outcome));
        }),
    }
}

fn single(outcome: Result<gio::File, glib::Error>) -> Vec<PathBuf> {
    outcome
        .ok()
        .and_then(|file| file.path())
        .into_iter()
        .collect()
}

fn paths_of(files: &gio::ListModel) -> Vec<PathBuf> {
    (0..files.n_items())
        .filter_map(|index| files.item(index).and_downcast::<gio::File>())
        .filter_map(|file| file.path())
        .collect()
}

fn filter_for(accepted: &[String]) -> Option<gtk::FileFilter> {
    if accepted.is_empty() {
        return None;
    }
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Supported files"));
    for pattern in accepted {
        if let Some(suffix) = pattern.strip_prefix('.') {
            filter.add_suffix(suffix);
        } else if pattern.contains('/') {
            filter.add_mime_type(pattern);
        } else {
            filter.add_pattern(pattern);
        }
    }
    Some(filter)
}

fn suggest_destination(dialog: &gtk::FileDialog, request: &FileRequest) {
    let suggested = request.suggested.clone().unwrap_or_default();
    if let Some(name) = suggested.file_name() {
        dialog.set_initial_name(Some(&name.to_string_lossy()));
    }
    let folder = suggested
        .parent()
        .filter(|folder| folder.is_absolute() && folder.is_dir())
        .map(PathBuf::from)
        .or_else(|| glib::user_special_dir(glib::UserDirectory::Downloads));
    if let Some(folder) = folder {
        dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
    }
}
