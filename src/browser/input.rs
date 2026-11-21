use std::cell::Cell;
use std::rc::{Rc, Weak};

use cef::{
    BrowserHost, CefString, CompositionUnderline, ImplBrowserHost, KeyEvent, KeyEventType,
    MouseButtonType, MouseEvent, Range,
};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use super::canvas::BrowserCanvas;
use super::keymap::{modifiers, utf16_unit, virtual_key};
use super::surface::SurfaceCore;

const WHEEL_STEP_PIXELS: f64 = 60.0;
const INVALID_RANGE: Range = Range {
    from: u32::MAX,
    to: u32::MAX,
};

struct InputState {
    core: Weak<SurfaceCore>,
    pointer: Cell<(f64, f64)>,
    composing: Cell<bool>,
}

impl InputState {
    fn host(&self) -> Option<BrowserHost> {
        self.core.upgrade().and_then(|core| core.host())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn mouse_event(&self, state: gdk::ModifierType) -> MouseEvent {
        let (x, y) = self.pointer.get();
        MouseEvent {
            x: x.round() as i32,
            y: y.round() as i32,
            modifiers: modifiers(state),
        }
    }
}

pub fn attach(canvas: &BrowserCanvas, core: Weak<SurfaceCore>, im: &gtk::IMMulticontext) {
    let state = Rc::new(InputState {
        core,
        pointer: Cell::new((0.0, 0.0)),
        composing: Cell::new(false),
    });
    im.set_client_widget(Some(canvas));
    attach_buttons(canvas, &state);
    attach_motion(canvas, &state);
    attach_wheel(canvas, &state);
    attach_keyboard(canvas, &state, im);
    attach_composition(&state, im);
    attach_focus(canvas, &state, im);
}

fn button_type(button: u32) -> Option<MouseButtonType> {
    match button {
        gdk::BUTTON_PRIMARY => Some(MouseButtonType::LEFT),
        gdk::BUTTON_MIDDLE => Some(MouseButtonType::MIDDLE),
        gdk::BUTTON_SECONDARY => Some(MouseButtonType::RIGHT),
        _ => None,
    }
}

fn attach_buttons(canvas: &BrowserCanvas, state: &Rc<InputState>) {
    let gesture = gtk::GestureClick::builder().button(0).build();
    let pressed_state = Rc::clone(state);
    gesture.connect_pressed(glib::clone!(
        #[weak]
        canvas,
        move |gesture, presses, x, y| {
            canvas.grab_focus();
            pressed_state.pointer.set((x, y));
            send_click(&pressed_state, gesture, presses, false);
        }
    ));
    let released_state = Rc::clone(state);
    gesture.connect_released(move |gesture, presses, x, y| {
        released_state.pointer.set((x, y));
        send_click(&released_state, gesture, presses, true);
    });
    canvas.add_controller(gesture);
}

fn send_click(state: &InputState, gesture: &gtk::GestureClick, presses: i32, released: bool) {
    let (Some(button), Some(host)) = (button_type(gesture.current_button()), state.host()) else {
        return;
    };
    let event = state.mouse_event(gesture.current_event_state());
    host.send_mouse_click_event(Some(&event), button, i32::from(released), presses);
}

fn attach_motion(canvas: &BrowserCanvas, state: &Rc<InputState>) {
    let motion = gtk::EventControllerMotion::new();
    let moving = Rc::clone(state);
    motion.connect_motion(move |controller, x, y| {
        moving.pointer.set((x, y));
        if let Some(host) = moving.host() {
            host.send_mouse_move_event(
                Some(&moving.mouse_event(controller.current_event_state())),
                0,
            );
        }
    });
    let leaving = Rc::clone(state);
    motion.connect_leave(move |controller| {
        if let Some(host) = leaving.host() {
            host.send_mouse_move_event(
                Some(&leaving.mouse_event(controller.current_event_state())),
                1,
            );
        }
    });
    canvas.add_controller(motion);
}

fn attach_wheel(canvas: &BrowserCanvas, state: &Rc<InputState>) {
    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    let scrolling = Rc::clone(state);
    wheel.connect_scroll(move |controller, dx, dy| {
        let Some(host) = scrolling.host() else {
            return glib::Propagation::Proceed;
        };
        let factor = if controller.unit() == gdk::ScrollUnit::Wheel {
            WHEEL_STEP_PIXELS
        } else {
            1.0
        };
        #[allow(clippy::cast_possible_truncation)]
        let (delta_x, delta_y) = ((-dx * factor).round() as i32, (-dy * factor).round() as i32);
        let event = scrolling.mouse_event(controller.current_event_state());
        host.send_mouse_wheel_event(Some(&event), delta_x, delta_y);
        glib::Propagation::Stop
    });
    canvas.add_controller(wheel);
}

fn key_event(kind: KeyEventType, key: gdk::Key, code: u32, state: gdk::ModifierType) -> KeyEvent {
    let unit = utf16_unit(key);
    KeyEvent {
        type_: kind,
        modifiers: modifiers(state),
        windows_key_code: virtual_key(key),
        native_key_code: i32::try_from(code).unwrap_or_default(),
        character: unit,
        unmodified_character: unit,
        ..Default::default()
    }
}

fn layout_key(canvas: &BrowserCanvas, key: gdk::Key, code: u32) -> gdk::Key {
    canvas
        .display()
        .translate_key(code, gdk::ModifierType::empty(), 0)
        .map_or(key, |(base, ..)| base)
}

fn attach_keyboard(canvas: &BrowserCanvas, state: &Rc<InputState>, im: &gtk::IMMulticontext) {
    let keys = gtk::EventControllerKey::new();
    keys.set_im_context(Some(im));
    let pressing = Rc::clone(state);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        canvas,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, code, modifier_state| {
            let Some(host) = pressing.host() else {
                return glib::Propagation::Proceed;
            };
            let base = layout_key(&canvas, key, code);
            let mut event = key_event(KeyEventType::RAWKEYDOWN, base, code, modifier_state);
            event.character = utf16_unit(key);
            host.send_key_event(Some(&event));
            if matches!(
                key,
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter
            ) {
                let mut enter = event.clone();
                enter.type_ = KeyEventType::CHAR;
                enter.windows_key_code = 0x0D;
                enter.character = 0x0D;
                enter.unmodified_character = 0x0D;
                host.send_key_event(Some(&enter));
            }
            glib::Propagation::Stop
        }
    ));
    let releasing = Rc::clone(state);
    keys.connect_key_released(glib::clone!(
        #[weak]
        canvas,
        move |_, key, code, modifier_state| {
            if let Some(host) = releasing.host() {
                let base = layout_key(&canvas, key, code);
                host.send_key_event(Some(&key_event(
                    KeyEventType::KEYUP,
                    base,
                    code,
                    modifier_state,
                )));
            }
        }
    ));
    canvas.add_controller(keys);
}

fn attach_composition(state: &Rc<InputState>, im: &gtk::IMMulticontext) {
    let committing = Rc::clone(state);
    im.connect_commit(move |_, committed| {
        let Some(host) = committing.host() else {
            return;
        };
        if committing.composing.replace(false) {
            host.ime_commit_text(Some(&CefString::from(committed)), Some(&INVALID_RANGE), 0);
            return;
        }
        for unit in committed.encode_utf16() {
            host.send_key_event(Some(&KeyEvent {
                type_: KeyEventType::CHAR,
                windows_key_code: i32::from(unit),
                character: unit,
                unmodified_character: unit,
                ..Default::default()
            }));
        }
    });

    let composing = Rc::clone(state);
    im.connect_preedit_changed(move |im| {
        let Some(host) = composing.host() else {
            return;
        };
        let (preedit, _, cursor) = im.preedit_string();
        if preedit.is_empty() {
            if composing.composing.replace(false) {
                host.ime_cancel_composition();
            }
            return;
        }
        composing.composing.set(true);
        let length = u32::try_from(preedit.encode_utf16().count()).unwrap_or_default();
        let caret = u32::try_from(cursor).unwrap_or(length);
        let underline = CompositionUnderline {
            range: Range {
                from: 0,
                to: length,
            },
            color: 0xFF00_0000,
            ..Default::default()
        };
        host.ime_set_composition(
            Some(&CefString::from(preedit.as_str())),
            Some(&[underline]),
            Some(&INVALID_RANGE),
            Some(&Range {
                from: caret,
                to: caret,
            }),
        );
    });
}

fn attach_focus(canvas: &BrowserCanvas, state: &Rc<InputState>, im: &gtk::IMMulticontext) {
    let focus = gtk::EventControllerFocus::new();
    let entering = Rc::clone(state);
    focus.connect_enter(glib::clone!(
        #[weak]
        im,
        move |_| {
            im.focus_in();
            if let Some(host) = entering.host() {
                host.set_focus(1);
            }
        }
    ));
    let leaving = Rc::clone(state);
    focus.connect_leave(glib::clone!(
        #[weak]
        im,
        move |_| {
            im.focus_out();
            if let Some(host) = leaving.host() {
                host.set_focus(0);
            }
        }
    ));
    canvas.add_controller(focus);
}
