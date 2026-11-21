use std::sync::atomic::{AtomicI32, Ordering};

use cef::{
    BrowserHost, CefString, ImplBrowserHost, ImplDictionaryValue, ImplListValue,
    dictionary_value_create, list_value_create,
};

static NEXT_CALL: AtomicI32 = AtomicI32::new(1);

pub fn emulate_color_scheme(host: &BrowserHost, dark: bool) {
    let (Some(mut feature), Some(mut features), Some(mut parameters)) = (
        dictionary_value_create(),
        list_value_create(),
        dictionary_value_create(),
    ) else {
        return;
    };
    let scheme = if dark { "dark" } else { "light" };
    feature.set_string(
        Some(&CefString::from("name")),
        Some(&CefString::from("prefers-color-scheme")),
    );
    feature.set_string(
        Some(&CefString::from("value")),
        Some(&CefString::from(scheme)),
    );
    features.set_dictionary(0, Some(&mut feature));
    parameters.set_list(Some(&CefString::from("features")), Some(&mut features));
    let call = NEXT_CALL.fetch_add(1, Ordering::Relaxed);
    let accepted = host.execute_dev_tools_method(
        call,
        Some(&CefString::from("Emulation.setEmulatedMedia")),
        Some(&mut parameters),
    );
    if accepted == 0 {
        tracing::debug!(scheme, "color scheme emulation not accepted");
    }
}
