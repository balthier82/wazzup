use std::cell::RefCell;
use std::time::Duration;

use gtk::glib;

const FALLBACK_INTERVAL: Duration = Duration::from_millis(50);
const LONGEST_DELAY_MS: i64 = 1000;

thread_local! {
    static PENDING: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

pub struct MessagePump;

impl MessagePump {
    pub fn request(delay_ms: i64) {
        glib::MainContext::default().invoke(move || Self::arm(delay_ms));
    }

    pub fn start_fallback() {
        glib::timeout_add_local(FALLBACK_INTERVAL, || {
            cef::do_message_loop_work();
            glib::ControlFlow::Continue
        });
    }

    fn arm(delay_ms: i64) {
        PENDING.with_borrow_mut(|pending| {
            if let Some(source) = pending.take() {
                source.remove();
            }
            let delay = Duration::from_millis(delay_ms.clamp(0, LONGEST_DELAY_MS).unsigned_abs());
            *pending = Some(glib::timeout_add_local_once(delay, || {
                PENDING.with_borrow_mut(Option::take);
                cef::do_message_loop_work();
            }));
        });
    }
}
