#![allow(clippy::transmute_ptr_to_ptr)]

use std::os::raw::c_int;

use cef::*;

use super::page_message::{CHANNEL, POST_FUNCTION};
use crate::config::HOME_HOST;
use crate::navigation::{Destination, NavigationPolicy};

const PAGE_BRIDGE_SCRIPT: &str = include_str!("scripts/page_bridge.js");

wrap_render_process_handler! {
    pub struct PageBridge;

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let (Some(frame), Some(context)) = (frame, context) else {
                return;
            };
            if belongs_to_whatsapp(frame) {
                install_bridge(context);
            }
        }
    }
}

fn frame_address(frame: &Frame) -> String {
    CefString::from(&frame.url()).to_string()
}

fn is_trusted(address: &str) -> bool {
    NavigationPolicy::new(HOME_HOST).classify(address) == Destination::Inside
}

fn belongs_to_whatsapp(frame: &Frame) -> bool {
    let address = frame_address(frame);
    if address.starts_with("https:") {
        return is_trusted(&address);
    }
    let is_blank = address.is_empty() || address == "about:blank";
    is_blank
        && frame
            .parent()
            .is_some_and(|parent| belongs_to_whatsapp(&parent))
}

fn install_bridge(context: &V8Context) {
    let Some(global) = context.global() else {
        return;
    };
    let mut handler = PostToBrowser::new();
    let mut function =
        v8_value_create_function(Some(&CefString::from(POST_FUNCTION)), Some(&mut handler));
    global.set_value_bykey(
        Some(&CefString::from(POST_FUNCTION)),
        function.as_mut(),
        V8Propertyattribute::default(),
    );
    let mut result = None;
    let mut failure = None;
    context.eval(
        Some(&CefString::from(PAGE_BRIDGE_SCRIPT)),
        None,
        0,
        Some(&mut result),
        Some(&mut failure),
    );
}

wrap_v8_handler! {
    struct PostToBrowser;

    impl V8Handler {
        fn execute(
            &self,
            _name: Option<&CefString>,
            _object: Option<&mut V8Value>,
            arguments: Option<&[Option<V8Value>]>,
            _retval: Option<&mut Option<V8Value>>,
            _exception: Option<&mut CefString>,
        ) -> c_int {
            let payload = arguments
                .and_then(<[Option<V8Value>]>::first)
                .and_then(Option::as_ref)
                .filter(|value| value.is_string() == 1)
                .map(|value| CefString::from(&value.string_value()));
            let frame = v8_context_get_current_context().and_then(|context| context.frame());
            let (Some(payload), Some(frame)) = (payload, frame) else {
                return 0;
            };
            let Some(mut message) = process_message_create(Some(&CefString::from(CHANNEL))) else {
                return 0;
            };
            if let Some(arguments) = message.argument_list() {
                arguments.set_string(0, Some(&payload));
            }
            frame.send_process_message(ProcessId::BROWSER, Some(&mut message));
            1
        }
    }
}
