use gtk::prelude::*;

use crate::i18n::t;

const GROUPS: [(&str, &[(&str, &str)]); 3] = [
    (
        "Application",
        &[
            ("<Control>comma", "Preferences"),
            ("<Control>question F1", "Keyboard shortcuts"),
            ("<Control>l", "Lock"),
            ("<Control>q", "Quit"),
        ],
    ),
    ("Page", &[("F5", "Reload"), ("<Control>r", "Reload")]),
    (
        "Editing",
        &[
            ("<Control>c", "Copy"),
            ("<Control>v", "Paste"),
            ("<Control>x", "Cut"),
            ("<Control>a", "Select all"),
        ],
    ),
];

pub fn window(parent: &impl IsA<gtk::Window>) -> gtk::ShortcutsWindow {
    let section = gtk::ShortcutsSection::builder()
        .section_name("main")
        .visible(true)
        .build();
    for (title, entries) in GROUPS {
        let group = gtk::ShortcutsGroup::builder().title(t(title)).build();
        for (accelerator, label) in entries {
            group.add_shortcut(
                &gtk::ShortcutsShortcut::builder()
                    .accelerator(*accelerator)
                    .title(t(label))
                    .build(),
            );
        }
        section.add_group(&group);
    }
    let window = gtk::ShortcutsWindow::builder()
        .transient_for(parent)
        .modal(true)
        .build();
    window.add_section(&section);
    window
}
