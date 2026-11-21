use cef::CursorType;

pub fn css_name(kind: CursorType) -> &'static str {
    match kind {
        CursorType::HAND => "pointer",
        CursorType::IBEAM => "text",
        CursorType::WAIT => "wait",
        CursorType::PROGRESS => "progress",
        CursorType::CROSS => "crosshair",
        CursorType::HELP => "help",
        CursorType::MOVE => "move",
        CursorType::NOTALLOWED => "not-allowed",
        CursorType::GRAB => "grab",
        CursorType::GRABBING => "grabbing",
        CursorType::CONTEXTMENU => "context-menu",
        CursorType::ZOOMIN => "zoom-in",
        CursorType::COLUMNRESIZE | CursorType::EASTWESTRESIZE => "ew-resize",
        CursorType::ROWRESIZE | CursorType::NORTHSOUTHRESIZE => "ns-resize",
        CursorType::NONE => "none",
        _ => "default",
    }
}
