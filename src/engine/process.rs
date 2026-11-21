#![allow(clippy::transmute_ptr_to_ptr)]

use cef::*;

use super::options::Switch;
use super::pump::MessagePump;
use super::renderer::PageBridge;

const BROWSER_SWITCHES: &[(&str, Option<&str>)] = &[
    ("autoplay-policy", Some("no-user-gesture-required")),
    ("enable-features", Some("WebRTCPipeWireCapturer")),
    ("disable-features", Some("Translate,MediaRouter")),
    ("hide-crash-restore-bubble", None),
    ("noerrdialogs", None),
];

fn display_server_switch() -> Option<(&'static str, Option<&'static str>)> {
    std::env::var_os("DISPLAY")
        .filter(|display| !display.is_empty())
        .map(|_| ("ozone-platform", Some("x11")))
}

wrap_app! {
    pub struct WazzupApp {
        extra_switches: Vec<Switch>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let is_browser = process_type.is_none_or(|kind| kind.to_string().is_empty());
            let Some(command_line) = command_line.filter(|_| is_browser) else {
                return;
            };
            let fixed = BROWSER_SWITCHES
                .iter()
                .chain(display_server_switch().iter())
                .map(|(name, value)| ((*name).to_owned(), value.map(str::to_owned)))
                .collect::<Vec<Switch>>();
            for (name, value) in fixed.iter().chain(self.extra_switches.iter()) {
                let name = CefString::from(name.as_str());
                match value {
                    Some(value) => command_line.append_switch_with_value(
                        Some(&name),
                        Some(&CefString::from(value.as_str())),
                    ),
                    None => command_line.append_switch(Some(&name)),
                }
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ProcessEvents::new())
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(PageBridge::new())
        }
    }
}

wrap_browser_process_handler! {
    struct ProcessEvents;

    impl BrowserProcessHandler {

        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            MessagePump::request(delay_ms);
        }
    }
}
