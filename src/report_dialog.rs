use adw::prelude::*;
use gtk::glib;

use crate::diagnostics::{Scrubber, environment, recent_log};
use crate::i18n::t;

const NEW_ISSUE: &str = "https://github.com/balthier82/wazzup/issues/new";
const LARGEST_LINK_BODY: usize = 6000;

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let scrubber = Scrubber::for_this_session();
    let report = scrubber.clean(&format!(
        "{}\n\n{}\n\n{}",
        environment(),
        t("Recent log:"),
        recent_log()
    ));
    let preview = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(8)
        .right_margin(8)
        .build();
    preview.buffer().set_text(&report);
    let scroller = gtk::ScrolledWindow::builder()
        .min_content_height(260)
        .min_content_width(420)
        .child(&preview)
        .css_classes(["card"])
        .build();
    let dialog = adw::AlertDialog::builder()
        .heading(t("Report a Problem"))
        .body(t(
            "This is everything that will be shared. Personal paths, phone numbers and email addresses have been hidden. Your chats are never included.",
        ))
        .extra_child(&scroller)
        .close_response("cancel")
        .default_response("open")
        .build();
    dialog.add_responses(&[
        ("cancel", &t("Cancel")),
        ("copy", &t("Copy")),
        ("open", &t("Open Issue Page")),
    ]);
    dialog.set_response_appearance("open", adw::ResponseAppearance::Suggested);
    let anchor = parent.as_ref().clone();
    dialog.connect_response(None, move |_, response| match response {
        "copy" => anchor.clipboard().set_text(&report),
        "open" => open_issue(&anchor, &report),
        _ => {}
    });
    dialog.present(Some(parent));
}

fn open_issue(anchor: &gtk::Widget, report: &str) {
    anchor.clipboard().set_text(report);
    let body = format!(
        "{}\n\n```\n{}\n```",
        t("Describe what happened:"),
        shorten(report, LARGEST_LINK_BODY)
    );
    let address = format!(
        "{NEW_ISSUE}?body={}",
        glib::Uri::escape_string(&body, None, false)
    );
    let parent = anchor.root().and_downcast::<gtk::Window>();
    gtk::UriLauncher::new(&address).launch(
        parent.as_ref(),
        None::<&gtk::gio::Cancellable>,
        |outcome| {
            if let Err(error) = outcome {
                tracing::warn!(%error, "cannot open the issue page");
            }
        },
    );
}

fn shorten(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut start = text.len() - limit;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

#[cfg(test)]
mod tests {
    use super::shorten;

    #[test]
    fn keeps_the_end_of_long_reports() {
        assert_eq!(shorten("abcdef", 3), "def");
        assert_eq!(shorten("abc", 10), "abc");
        assert_eq!(shorten("ààà", 3), "à");
    }
}
