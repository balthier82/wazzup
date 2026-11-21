use serde::Deserialize;

pub const CHANNEL: &str = "wazzup.page";
pub const POST_FUNCTION: &str = "__wazzupPost";
pub const ACTIVATE_FUNCTION: &str = "__wazzupActivate";
pub const VEIL_FUNCTION: &str = "__wazzupVeil";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PageMessage {
    Notification {
        id: u64,
        title: String,
        #[serde(default)]
        body: String,
    },
    NotificationClosed {
        id: u64,
    },
    Connection {
        online: bool,
    },
}

impl PageMessage {
    pub fn parse(raw: &str) -> Option<Self> {
        serde_json::from_str(raw).ok()
    }

    pub fn activation_script(id: u64) -> String {
        format!("globalThis.{ACTIVATE_FUNCTION}?.({id});")
    }

    pub fn veil_script(pixels: u32) -> String {
        format!("globalThis.{VEIL_FUNCTION}?.({pixels});")
    }
}

#[cfg(test)]
mod tests {
    use super::PageMessage;

    #[test]
    fn parses_notification() {
        let message =
            PageMessage::parse(r#"{"kind":"notification","id":4,"title":"Anna","body":"Hi"}"#);
        assert_eq!(
            message,
            Some(PageMessage::Notification {
                id: 4,
                title: "Anna".to_owned(),
                body: "Hi".to_owned()
            })
        );
    }

    #[test]
    fn parses_notification_without_body() {
        let message = PageMessage::parse(r#"{"kind":"notification","id":1,"title":"Group"}"#);
        assert!(matches!(message, Some(PageMessage::Notification { body, .. }) if body.is_empty()));
    }

    #[test]
    fn parses_closure() {
        assert_eq!(
            PageMessage::parse(r#"{"kind":"notification-closed","id":9}"#),
            Some(PageMessage::NotificationClosed { id: 9 })
        );
    }

    #[test]
    fn parses_connection_changes() {
        assert_eq!(
            PageMessage::parse(r#"{"kind":"connection","online":false}"#),
            Some(PageMessage::Connection { online: false })
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(PageMessage::parse("{\"kind\":\"unknown\"}"), None);
        assert_eq!(PageMessage::parse("not json"), None);
    }

    #[test]
    fn builds_veil_script() {
        assert_eq!(PageMessage::veil_script(6), "globalThis.__wazzupVeil?.(6);");
    }

    #[test]
    fn builds_activation_script() {
        assert_eq!(
            PageMessage::activation_script(12),
            "globalThis.__wazzupActivate?.(12);"
        );
    }
}
