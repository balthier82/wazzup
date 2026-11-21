#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnreadCount(u32);

impl UnreadCount {
    pub fn from_page_title(title: &str) -> Self {
        let Some(rest) = title.trim_start().strip_prefix('(') else {
            return Self::default();
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let tail = &rest[digits.len()..];
        if !(tail.starts_with(')') || tail.starts_with("+)")) {
            return Self::default();
        }
        digits.parse().map_or_default(Self)
    }

    pub fn value(self) -> u32 {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::UnreadCount;

    #[test]
    fn reads_plain_count() {
        assert_eq!(UnreadCount::from_page_title("(7) WhatsApp").value(), 7);
    }

    #[test]
    fn reads_capped_count() {
        assert_eq!(UnreadCount::from_page_title(" (99+) WhatsApp").value(), 99);
    }

    #[test]
    fn ignores_titles_without_count() {
        assert!(UnreadCount::from_page_title("WhatsApp").is_zero());
        assert!(UnreadCount::from_page_title("(abc) WhatsApp").is_zero());
        assert!(UnreadCount::from_page_title("() WhatsApp").is_zero());
        assert!(UnreadCount::from_page_title("(12 WhatsApp").is_zero());
    }

    #[test]
    fn ignores_overflowing_count() {
        assert!(UnreadCount::from_page_title("(99999999999) WhatsApp").is_zero());
    }
}
