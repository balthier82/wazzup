use cef::sys::cef_event_flags_t;
use gtk::gdk;
use gtk::glib::translate::IntoGlib;

const FUNCTION_KEY_BASE: i32 = 0x70;
const NUMPAD_DIGIT_BASE: i32 = 0x60;

pub fn virtual_key(key: gdk::Key) -> i32 {
    if let Some(code) = named_key(key) {
        return code;
    }
    if let Some(offset) = offset_within(key, gdk::Key::F1, gdk::Key::F24) {
        return FUNCTION_KEY_BASE + offset;
    }
    if let Some(offset) = offset_within(key, gdk::Key::KP_0, gdk::Key::KP_9) {
        return NUMPAD_DIGIT_BASE + offset;
    }
    match key.to_lower().to_unicode() {
        Some(letter) if letter.is_ascii_alphabetic() => ascii_code(letter.to_ascii_uppercase()),
        Some(digit) if digit.is_ascii_digit() => ascii_code(digit),
        Some(symbol) => symbol_key(symbol),
        None => 0,
    }
}

pub fn modifiers(state: gdk::ModifierType) -> u32 {
    [
        (
            gdk::ModifierType::SHIFT_MASK,
            cef_event_flags_t::EVENTFLAG_SHIFT_DOWN,
        ),
        (
            gdk::ModifierType::CONTROL_MASK,
            cef_event_flags_t::EVENTFLAG_CONTROL_DOWN,
        ),
        (
            gdk::ModifierType::ALT_MASK,
            cef_event_flags_t::EVENTFLAG_ALT_DOWN,
        ),
        (
            gdk::ModifierType::SUPER_MASK,
            cef_event_flags_t::EVENTFLAG_COMMAND_DOWN,
        ),
        (
            gdk::ModifierType::LOCK_MASK,
            cef_event_flags_t::EVENTFLAG_CAPS_LOCK_ON,
        ),
        (
            gdk::ModifierType::BUTTON1_MASK,
            cef_event_flags_t::EVENTFLAG_LEFT_MOUSE_BUTTON,
        ),
        (
            gdk::ModifierType::BUTTON2_MASK,
            cef_event_flags_t::EVENTFLAG_MIDDLE_MOUSE_BUTTON,
        ),
        (
            gdk::ModifierType::BUTTON3_MASK,
            cef_event_flags_t::EVENTFLAG_RIGHT_MOUSE_BUTTON,
        ),
    ]
    .into_iter()
    .filter(|(mask, _)| state.contains(*mask))
    .fold(0, |flags, (_, flag)| flags | flag.0)
}

pub fn utf16_unit(key: gdk::Key) -> u16 {
    key.to_unicode()
        .and_then(|symbol| u16::try_from(u32::from(symbol)).ok())
        .unwrap_or_default()
}

fn ascii_code(symbol: char) -> i32 {
    i32::try_from(u32::from(symbol)).unwrap_or_default()
}

fn offset_within(key: gdk::Key, first: gdk::Key, last: gdk::Key) -> Option<i32> {
    let raw = key.into_glib();
    (first.into_glib()..=last.into_glib())
        .contains(&raw)
        .then(|| i32::try_from(raw - first.into_glib()).unwrap_or_default())
}

fn named_key(key: gdk::Key) -> Option<i32> {
    let code = match key {
        gdk::Key::BackSpace => 0x08,
        gdk::Key::Tab | gdk::Key::ISO_Left_Tab => 0x09,
        gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => 0x0D,
        gdk::Key::Shift_L | gdk::Key::Shift_R => 0x10,
        gdk::Key::Control_L | gdk::Key::Control_R => 0x11,
        gdk::Key::Alt_L | gdk::Key::Alt_R | gdk::Key::ISO_Level3_Shift => 0x12,
        gdk::Key::Pause => 0x13,
        gdk::Key::Caps_Lock => 0x14,
        gdk::Key::Escape => 0x1B,
        gdk::Key::space | gdk::Key::KP_Space => 0x20,
        gdk::Key::Page_Up | gdk::Key::KP_Page_Up => 0x21,
        gdk::Key::Page_Down | gdk::Key::KP_Page_Down => 0x22,
        gdk::Key::End | gdk::Key::KP_End => 0x23,
        gdk::Key::Home | gdk::Key::KP_Home => 0x24,
        gdk::Key::Left | gdk::Key::KP_Left => 0x25,
        gdk::Key::Up | gdk::Key::KP_Up => 0x26,
        gdk::Key::Right | gdk::Key::KP_Right => 0x27,
        gdk::Key::Down | gdk::Key::KP_Down => 0x28,
        gdk::Key::Print => 0x2C,
        gdk::Key::Insert | gdk::Key::KP_Insert => 0x2D,
        gdk::Key::Delete | gdk::Key::KP_Delete => 0x2E,
        gdk::Key::Super_L | gdk::Key::Meta_L => 0x5B,
        gdk::Key::Super_R | gdk::Key::Meta_R => 0x5C,
        gdk::Key::Menu => 0x5D,
        gdk::Key::KP_Multiply => 0x6A,
        gdk::Key::KP_Add => 0x6B,
        gdk::Key::KP_Subtract => 0x6D,
        gdk::Key::KP_Decimal => 0x6E,
        gdk::Key::KP_Divide => 0x6F,
        gdk::Key::Num_Lock => 0x90,
        gdk::Key::Scroll_Lock => 0x91,
        _ => return None,
    };
    Some(code)
}

fn symbol_key(symbol: char) -> i32 {
    match symbol {
        ';' => 0xBA,
        '=' => 0xBB,
        ',' => 0xBC,
        '-' => 0xBD,
        '.' => 0xBE,
        '/' => 0xBF,
        '`' => 0xC0,
        '[' => 0xDB,
        '\\' => 0xDC,
        ']' => 0xDD,
        '\'' => 0xDE,
        '<' => 0xE2,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{modifiers, virtual_key};
    use cef::sys::cef_event_flags_t;
    use gtk::gdk;

    #[test]
    fn maps_letters_regardless_of_case() {
        assert_eq!(virtual_key(gdk::Key::a), 0x41);
        assert_eq!(virtual_key(gdk::Key::Z), 0x5A);
    }

    #[test]
    fn maps_digits_and_function_keys() {
        assert_eq!(virtual_key(gdk::Key::_7), 0x37);
        assert_eq!(virtual_key(gdk::Key::F1), 0x70);
        assert_eq!(virtual_key(gdk::Key::F12), 0x7B);
        assert_eq!(virtual_key(gdk::Key::KP_3), 0x63);
    }

    #[test]
    fn maps_editing_keys() {
        assert_eq!(virtual_key(gdk::Key::Return), 0x0D);
        assert_eq!(virtual_key(gdk::Key::BackSpace), 0x08);
        assert_eq!(virtual_key(gdk::Key::Escape), 0x1B);
        assert_eq!(virtual_key(gdk::Key::Left), 0x25);
    }

    #[test]
    fn maps_punctuation() {
        assert_eq!(virtual_key(gdk::Key::comma), 0xBC);
        assert_eq!(virtual_key(gdk::Key::apostrophe), 0xDE);
    }

    #[test]
    fn combines_modifier_flags() {
        let state = gdk::ModifierType::SHIFT_MASK | gdk::ModifierType::CONTROL_MASK;
        let expected =
            cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0 | cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;
        assert_eq!(modifiers(state), expected);
        assert_eq!(modifiers(gdk::ModifierType::empty()), 0);
    }
}
