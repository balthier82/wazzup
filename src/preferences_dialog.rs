use std::rc::Rc;

use adw::prelude::*;

use crate::i18n::{Language, t, t_with};
use crate::lock::{AppLock, MINIMUM_LENGTH, PasscodeProblem};
use crate::preferences::{
    CloseAction, NotificationDetail, PreferenceStore, ThemeChoice, TrayStyle, Veil,
};
use crate::proxy::{ProxyKind, ProxyMode, ProxyPassword};

const THEMES: [(ThemeChoice, &str); 3] = [
    (ThemeChoice::System, "Follow system"),
    (ThemeChoice::Light, "Light"),
    (ThemeChoice::Dark, "Dark"),
];

const TRAY_STYLES: [(TrayStyle, &str); 2] = [
    (TrayStyle::Light, "Light, for dark top bars"),
    (TrayStyle::Dark, "Dark, for light top bars"),
];

const VEILS: [(Veil, &str); 4] = [
    (Veil::Off, "Off"),
    (Veil::Light, "Light"),
    (Veil::Medium, "Medium"),
    (Veil::Strong, "Strong"),
];

const RESTART_NOTE: &str = "Changes apply the next time Wazzup starts.";

const PROXY_MODES: [(ProxyMode, &str); 3] = [
    (ProxyMode::System, "Use system settings"),
    (ProxyMode::Direct, "No proxy"),
    (ProxyMode::Manual, "Manual"),
];

const PROXY_KINDS: [(ProxyKind, &str); 2] =
    [(ProxyKind::Http, "HTTP"), (ProxyKind::Socks5, "SOCKS5")];

const IDLE_MINUTES: [(u32, &str); 6] = [
    (0, "Never"),
    (1, "After 1 minute"),
    (5, "After 5 minutes"),
    (15, "After 15 minutes"),
    (30, "After 30 minutes"),
    (60, "After 1 hour"),
];

const DETAILS: [(NotificationDetail, &str); 3] = [
    (NotificationDetail::SenderAndMessage, "Sender and message"),
    (NotificationDetail::SenderOnly, "Sender only"),
    (NotificationDetail::Nothing, "Only “New message”"),
];

pub fn build(
    store: &Rc<PreferenceStore>,
    lock: &Rc<AppLock>,
    password: &Rc<ProxyPassword>,
) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::builder()
        .title(t("Preferences"))
        .search_enabled(false)
        .build();
    dialog.add(&general_page(store));
    dialog.add(&appearance_page(store));
    dialog.add(&notifications_page(store));
    dialog.add(&privacy_page(store, lock));
    dialog.add(&network_page(store, password));
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
        .title(t(title))
        .subtitle(t(subtitle))
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
    let labels: Vec<String> = choices.iter().map(|(_, label)| t(label)).collect();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let current = read(&store.snapshot());
    let selected = choices
        .iter()
        .position(|(choice, _)| *choice == current)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or_default();
    let row = adw::ComboRow::builder()
        .title(t(title))
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
    let group = adw::PreferencesGroup::builder().title(t(title)).build();
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
        &[
            group(
                "Behaviour",
                &[
                    background.upcast_ref(),
                    start_hidden.upcast_ref(),
                    login.upcast_ref(),
                ],
            ),
            language_group(store),
            spelling_group(store),
            advanced_group(store),
        ],
    )
}

fn language_group(store: &Rc<PreferenceStore>) -> adw::PreferencesGroup {
    let language = choice_row(
        store,
        "Interface language",
        &Language::CHOICES,
        |preferences| preferences.language,
        |preferences, language| preferences.language = language,
    );
    let interface = group("Language", &[language.upcast_ref()]);
    interface.set_description(Some(&t(RESTART_NOTE)));
    interface
}

fn advanced_group(store: &Rc<PreferenceStore>) -> adw::PreferencesGroup {
    let acceleration = switch_row(
        store,
        "Hardware acceleration",
        "Turn off if the window stays black or flickers",
        |preferences| preferences.hardware_acceleration,
        |preferences, active| preferences.hardware_acceleration = active,
    );
    let advanced = group("Advanced", &[acceleration.upcast_ref()]);
    advanced.set_description(Some(&t(RESTART_NOTE)));
    advanced
}

fn spelling_group(store: &Rc<PreferenceStore>) -> adw::PreferencesGroup {
    let enabled = switch_row(
        store,
        "Check spelling",
        "Underline mistakes while you type",
        |preferences| preferences.spell_check,
        |preferences, active| preferences.spell_check = active,
    );
    let languages = adw::ExpanderRow::builder()
        .title(t("Languages"))
        .subtitle(t("Right-click a highlighted word for suggestions"))
        .build();
    let chosen = store.snapshot().spell_languages;
    for (code, name) in crate::spelling::DICTIONARIES {
        let check = gtk::CheckButton::builder()
            .active(chosen.iter().any(|language| language == code))
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder()
            .title(name)
            .activatable_widget(&check)
            .build();
        row.add_prefix(&check);
        let store = Rc::clone(store);
        check.connect_toggled(move |check| {
            let active = check.is_active();
            store.update(|preferences| {
                preferences
                    .spell_languages
                    .retain(|language| language != code);
                if active {
                    preferences.spell_languages.push(code.to_owned());
                }
            });
        });
        languages.add_row(&row);
    }
    enabled
        .bind_property("active", &languages, "sensitive")
        .sync_create()
        .build();
    let spelling = group("Spelling", &[enabled.upcast_ref(), languages.upcast_ref()]);
    spelling.set_description(Some(&t(RESTART_NOTE)));
    spelling
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasscodeTask {
    Create,
    Change,
    Remove,
}

fn lock_group(store: &Rc<PreferenceStore>, lock: &Rc<AppLock>) -> adw::PreferencesGroup {
    let passcode = adw::ActionRow::builder().title(t("Passcode")).build();
    let edit = gtk::Button::builder().valign(gtk::Align::Center).build();
    let remove = gtk::Button::builder()
        .label(t("Remove"))
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    passcode.add_suffix(&remove);
    passcode.add_suffix(&edit);

    let on_start = switch_row(
        store,
        "Lock on start",
        "Ask for the passcode every time Wazzup opens",
        |preferences| preferences.lock_on_start,
        |preferences, active| preferences.lock_on_start = active,
    );
    let when_hidden = switch_row(
        store,
        "Lock when hidden",
        "Lock as soon as the window is closed or hidden",
        |preferences| preferences.lock_when_hidden,
        |preferences, active| preferences.lock_when_hidden = active,
    );
    let idle = choice_row(
        store,
        "Lock when idle",
        &IDLE_MINUTES,
        |preferences| preferences.lock_after_minutes,
        |preferences, minutes| preferences.lock_after_minutes = minutes,
    );

    let refresh: Rc<dyn Fn()> = {
        let lock = Rc::clone(lock);
        let (passcode, edit, remove) = (passcode.clone(), edit.clone(), remove.clone());
        let options = [
            on_start.clone().upcast::<gtk::Widget>(),
            when_hidden.clone().upcast(),
            idle.clone().upcast(),
        ];
        Rc::new(move || {
            let protected = lock.has_passcode();
            let (subtitle, action) = if protected {
                (t("Wazzup can be locked"), t("Change…"))
            } else {
                (t("Set a passcode to lock Wazzup"), t("Set…"))
            };
            passcode.set_subtitle(&subtitle);
            edit.set_label(&action);
            remove.set_visible(protected);
            for option in &options {
                option.set_sensitive(protected);
            }
        })
    };
    refresh();
    {
        let lock = Rc::clone(lock);
        let refresh = Rc::clone(&refresh);
        edit.connect_clicked(move |button| {
            let task = if lock.has_passcode() {
                PasscodeTask::Change
            } else {
                PasscodeTask::Create
            };
            ask_passcode(button, &lock, task, Rc::clone(&refresh));
        });
    }
    {
        let lock = Rc::clone(lock);
        let refresh = Rc::clone(&refresh);
        remove.connect_clicked(move |button| {
            ask_passcode(button, &lock, PasscodeTask::Remove, Rc::clone(&refresh));
        });
    }

    group(
        "App lock",
        &[
            passcode.upcast_ref(),
            on_start.upcast_ref(),
            when_hidden.upcast_ref(),
            idle.upcast_ref(),
        ],
    )
}

fn privacy_page(store: &Rc<PreferenceStore>, lock: &Rc<AppLock>) -> adw::PreferencesPage {
    let microphone = switch_row(
        store,
        "Microphone",
        "Voice messages and calls",
        |preferences| preferences.allow_microphone,
        |preferences, active| preferences.allow_microphone = active,
    );
    let camera = switch_row(
        store,
        "Camera",
        "Video calls",
        |preferences| preferences.allow_camera,
        |preferences, active| preferences.allow_camera = active,
    );
    let screen = switch_row(
        store,
        "Screen sharing",
        "Share your screen during calls",
        |preferences| preferences.allow_screen_sharing,
        |preferences, active| preferences.allow_screen_sharing = active,
    );
    let veil = choice_row(
        store,
        "Blur chats",
        &VEILS,
        |preferences| preferences.veil,
        |preferences, level| preferences.veil = level,
    );
    veil.set_subtitle(&t("Hide names and messages until you point at them"));

    page(
        "Privacy",
        "security-high-symbolic",
        &[
            lock_group(store, lock),
            group(
                "WhatsApp may use",
                &[
                    microphone.upcast_ref(),
                    camera.upcast_ref(),
                    screen.upcast_ref(),
                ],
            ),
            group("Screen privacy", &[veil.upcast_ref()]),
        ],
    )
}

fn secret_row(title: &str) -> adw::PasswordEntryRow {
    adw::PasswordEntryRow::builder().title(t(title)).build()
}

fn ask_passcode(
    anchor: &gtk::Button,
    lock: &Rc<AppLock>,
    task: PasscodeTask,
    refresh: Rc<dyn Fn()>,
) {
    let current = (task != PasscodeTask::Create).then(|| secret_row("Current passcode"));
    let fresh = (task != PasscodeTask::Remove).then(|| secret_row("New passcode"));
    let confirmation = (task != PasscodeTask::Remove).then(|| secret_row("Repeat new passcode"));
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    for row in [&current, &fresh, &confirmation].into_iter().flatten() {
        list.append(row);
    }
    let length_rule = t_with(
        "Use at least {length} characters.",
        &[("length", &MINIMUM_LENGTH)],
    );
    let (heading, body, action) = match task {
        PasscodeTask::Create => (t("Set Passcode"), length_rule, t("Set")),
        PasscodeTask::Change => (t("Change Passcode"), length_rule, t("Change")),
        PasscodeTask::Remove => (
            t("Remove Passcode"),
            t("Wazzup will no longer be locked."),
            t("Remove"),
        ),
    };
    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .body(body)
        .extra_child(&list)
        .close_response("cancel")
        .default_response("apply")
        .build();
    dialog.add_responses(&[("cancel", &t("Cancel")), ("apply", &action)]);
    dialog.set_response_appearance(
        "apply",
        if task == PasscodeTask::Remove {
            adw::ResponseAppearance::Destructive
        } else {
            adw::ResponseAppearance::Suggested
        },
    );
    let lock = Rc::clone(lock);
    let anchor_widget = anchor.clone();
    dialog.connect_response(Some("apply"), move |_, _| {
        let text =
            |row: &Option<adw::PasswordEntryRow>| row.as_ref().map(|row| row.text().to_string());
        let outcome = match task {
            PasscodeTask::Remove => lock.remove_passcode(&text(&current).unwrap_or_default()),
            PasscodeTask::Create | PasscodeTask::Change => lock.set_passcode(
                text(&current).as_deref(),
                &text(&fresh).unwrap_or_default(),
                &text(&confirmation).unwrap_or_default(),
            ),
        };
        refresh();
        if let Err(problem) = outcome {
            explain_problem(&anchor_widget, problem);
        }
    });
    dialog.present(Some(anchor));
}

fn explain_problem(anchor: &gtk::Button, problem: PasscodeProblem) {
    let body = match problem {
        PasscodeProblem::TooShort => t_with(
            "The passcode needs at least {length} characters.",
            &[("length", &MINIMUM_LENGTH)],
        ),
        PasscodeProblem::Mismatch => t("The two new passcodes are different."),
        PasscodeProblem::WrongCurrent => {
            t("The current passcode is wrong, or you need to wait before trying again.")
        }
        PasscodeProblem::Unavailable => t("The passcode could not be saved."),
    };
    let dialog = adw::AlertDialog::builder()
        .heading(t("Passcode Not Changed"))
        .body(body)
        .build();
    dialog.add_response("ok", &t("OK"));
    dialog.present(Some(anchor));
}

fn text_row(
    store: &Rc<PreferenceStore>,
    title: &str,
    read: fn(&crate::preferences::Preferences) -> String,
    write: fn(&mut crate::preferences::Preferences, String),
) -> adw::EntryRow {
    let row = adw::EntryRow::builder()
        .title(t(title))
        .text(read(&store.snapshot()))
        .show_apply_button(true)
        .build();
    let store = Rc::clone(store);
    row.connect_apply(move |row| {
        let value = row.text().trim().to_owned();
        store.update(|preferences| write(preferences, value));
    });
    row
}

fn network_page(store: &Rc<PreferenceStore>, password: &Rc<ProxyPassword>) -> adw::PreferencesPage {
    let mode = choice_row(
        store,
        "Proxy",
        &PROXY_MODES,
        |preferences| preferences.proxy.mode,
        |preferences, mode| preferences.proxy.mode = mode,
    );
    let kind = choice_row(
        store,
        "Type",
        &PROXY_KINDS,
        |preferences| preferences.proxy.kind,
        |preferences, kind| preferences.proxy.kind = kind,
    );
    let host = text_row(
        store,
        "Server",
        |preferences| preferences.proxy.host.clone(),
        |preferences, host| preferences.proxy.host = host,
    );
    let port = adw::SpinRow::with_range(1.0, 65535.0, 1.0);
    port.set_title(&t("Port"));
    port.set_value(f64::from(store.snapshot().proxy.port));
    {
        let store = Rc::clone(store);
        port.connect_value_notify(move |row| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let value = row.value().clamp(1.0, 65535.0).round() as u16;
            store.update(|preferences| preferences.proxy.port = value);
        });
    }
    let username = text_row(
        store,
        "Username",
        |preferences| preferences.proxy.username.clone(),
        |preferences, username| preferences.proxy.username = username,
    );
    let secret = adw::PasswordEntryRow::builder()
        .title(t("Password"))
        .show_apply_button(true)
        .text(password.current().unwrap_or_default())
        .build();
    {
        let password = Rc::clone(password);
        secret.connect_apply(move |row| password.store(&row.text()));
    }
    let manual_rows: [gtk::Widget; 5] = [
        kind.clone().upcast(),
        host.clone().upcast(),
        port.clone().upcast(),
        username.clone().upcast(),
        secret.clone().upcast(),
    ];
    let refresh = move |mode: &adw::ComboRow| {
        let manual = PROXY_MODES
            .get(usize::try_from(mode.selected()).unwrap_or_default())
            .is_some_and(|(choice, _)| *choice == ProxyMode::Manual);
        for row in &manual_rows {
            row.set_sensitive(manual);
        }
    };
    refresh(&mode);
    mode.connect_selected_notify(refresh);
    page(
        "Network",
        "network-wired-symbolic",
        &[
            {
                let connection = group("Connection", &[mode.upcast_ref()]);
                connection.set_description(Some(&t(RESTART_NOTE)));
                connection
            },
            group(
                "Manual proxy",
                &[
                    kind.upcast_ref(),
                    host.upcast_ref(),
                    port.upcast_ref(),
                    username.upcast_ref(),
                    secret.upcast_ref(),
                ],
            ),
        ],
    )
}
