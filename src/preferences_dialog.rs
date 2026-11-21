use std::rc::Rc;

use adw::prelude::*;

use crate::preferences::{
    CloseAction, NotificationDetail, PreferenceStore, ThemeChoice, TrayStyle,
};

const THEMES: [(ThemeChoice, &str); 3] = [
    (ThemeChoice::System, "Follow system"),
    (ThemeChoice::Light, "Light"),
    (ThemeChoice::Dark, "Dark"),
];

const TRAY_STYLES: [(TrayStyle, &str); 2] = [
    (TrayStyle::Light, "Light, for dark top bars"),
    (TrayStyle::Dark, "Dark, for light top bars"),
];

const DETAILS: [(NotificationDetail, &str); 3] = [
    (NotificationDetail::SenderAndMessage, "Sender and message"),
    (NotificationDetail::SenderOnly, "Sender only"),
    (NotificationDetail::Nothing, "Only “New message”"),
];

pub fn build(store: &Rc<PreferenceStore>) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::builder()
        .title("Preferences")
        .search_enabled(false)
        .build();
    dialog.add(&general_page(store));
    dialog.add(&appearance_page(store));
    dialog.add(&notifications_page(store));
    dialog
}

fn switch_row(
    store: &Rc<PreferenceStore>,
    title: &str,
    subtitle: &str,
    read: fn(&crate::preferences::Preferences) -> bool,
    write: fn(&mut crate::preferences::Preferences, bool),
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(read(&store.snapshot()))
        .build();
    let store = Rc::clone(store);
    row.connect_active_notify(move |row| {
        let active = row.is_active();
        store.update(|preferences| write(preferences, active));
    });
    row
}

fn choice_row<T: Copy + PartialEq + 'static>(
    store: &Rc<PreferenceStore>,
    title: &str,
    choices: &'static [(T, &'static str)],
    read: fn(&crate::preferences::Preferences) -> T,
    write: fn(&mut crate::preferences::Preferences, T),
) -> adw::ComboRow {
    let labels: Vec<&str> = choices.iter().map(|(_, label)| *label).collect();
    let current = read(&store.snapshot());
    let selected = choices
        .iter()
        .position(|(choice, _)| *choice == current)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or_default();
    let row = adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(&labels))
        .selected(selected)
        .build();
    let store = Rc::clone(store);
    row.connect_selected_notify(move |row| {
        let index = usize::try_from(row.selected()).unwrap_or_default();
        if let Some((choice, _)) = choices.get(index) {
            let choice = *choice;
            store.update(|preferences| write(preferences, choice));
        }
    });
    row
}

fn page(title: &str, icon: &str, groups: &[adw::PreferencesGroup]) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(title)
        .icon_name(icon)
        .build();
    for group in groups {
        page.add(group);
    }
    page
}

fn group(title: &str, rows: &[&gtk::Widget]) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    for row in rows {
        group.add(*row);
    }
    group
}

fn general_page(store: &Rc<PreferenceStore>) -> adw::PreferencesPage {
    let background = switch_row(
        store,
        "Keep running when closed",
        "Closing the window keeps Wazzup available for notifications",
        |preferences| preferences.close_action == CloseAction::KeepInBackground,
        |preferences, active| {
            preferences.close_action = if active {
                CloseAction::KeepInBackground
            } else {
                CloseAction::Quit
            };
        },
    );
    let start_hidden = switch_row(
        store,
        "Start in background",
        "Open without showing the window",
        |preferences| preferences.start_in_background,
        |preferences, active| preferences.start_in_background = active,
    );
    let login = switch_row(
        store,
        "Launch at login",
        "Start Wazzup in background when you sign in",
        |preferences| preferences.launch_at_login,
        |preferences, active| preferences.launch_at_login = active,
    );
    page(
        "General",
        "preferences-system-symbolic",
        &[group(
            "Behaviour",
            &[
                background.upcast_ref(),
                start_hidden.upcast_ref(),
                login.upcast_ref(),
            ],
        )],
    )
}

fn appearance_page(store: &Rc<PreferenceStore>) -> adw::PreferencesPage {
    let theme = choice_row(
        store,
        "Style",
        &THEMES,
        |preferences| preferences.theme,
        |preferences, choice| preferences.theme = choice,
    );
    let tray_icon = switch_row(
        store,
        "Show in the top bar",
        "Needs the AppIndicator extension on GNOME",
        |preferences| preferences.tray_icon,
        |preferences, active| preferences.tray_icon = active,
    );
    let tray_style = choice_row(
        store,
        "Icon colour",
        &TRAY_STYLES,
        |preferences| preferences.tray_style,
        |preferences, choice| preferences.tray_style = choice,
    );
    tray_icon
        .bind_property("active", &tray_style, "sensitive")
        .sync_create()
        .build();
    page(
        "Appearance",
        "applications-graphics-symbolic",
        &[
            group("Theme", &[theme.upcast_ref()]),
            group(
                "Top bar icon",
                &[tray_icon.upcast_ref(), tray_style.upcast_ref()],
            ),
        ],
    )
}

fn notifications_page(store: &Rc<PreferenceStore>) -> adw::PreferencesPage {
    let enabled = switch_row(
        store,
        "Show notifications",
        "Announce new messages and calls",
        |preferences| preferences.notifications,
        |preferences, active| preferences.notifications = active,
    );
    let detail = choice_row(
        store,
        "Content",
        &DETAILS,
        |preferences| preferences.notification_detail,
        |preferences, choice| preferences.notification_detail = choice,
    );
    enabled
        .bind_property("active", &detail, "sensitive")
        .sync_create()
        .build();
    let badge = switch_row(
        store,
        "Unread badge",
        "Show the number of unread chats on the app icon",
        |preferences| preferences.unread_badge,
        |preferences, active| preferences.unread_badge = active,
    );
    page(
        "Notifications",
        "preferences-system-notifications-symbolic",
        &[
            group("Messages", &[enabled.upcast_ref(), detail.upcast_ref()]),
            group("App icon", &[badge.upcast_ref()]),
        ],
    )
}
