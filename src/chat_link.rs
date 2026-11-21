use gtk::glib::{Uri, UriFlags};

use crate::config::HOME_URL;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLink {
    phone: String,
    text: Option<String>,
}

impl ChatLink {
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if let Some(phone) = Self::phone_only(trimmed) {
            return Some(Self { phone, text: None });
        }
        let uri = Uri::parse(trimmed, UriFlags::ENCODED_QUERY).ok()?;
        let scheme = uri.scheme().to_ascii_lowercase();
        let host = uri.host().map_or_default(|host| host.to_ascii_lowercase());
        let path = uri.path().to_string();
        let query = query_pairs(&uri);
        let text = lookup(&query, "text").filter(|text| !text.is_empty());

        let phone = match (scheme.as_str(), host.as_str()) {
            ("whatsapp", "send") => lookup(&query, "phone"),
            ("https" | "http", "wa.me" | "www.wa.me") => Some(path.trim_matches('/').to_owned()),
            ("https" | "http", "api.whatsapp.com" | "web.whatsapp.com")
                if path.trim_end_matches('/') == "/send" =>
            {
                lookup(&query, "phone")
            }
            _ => None,
        }?;
        let phone = digits_of(&phone)?;
        Some(Self { phone, text })
    }

    pub fn address(&self) -> String {
        let mut address = format!("{HOME_URL}send?phone={}", self.phone);
        if let Some(text) = &self.text {
            address.push_str("&text=");
            address.push_str(&Uri::escape_string(text, None, false));
        }
        address
    }

    fn phone_only(input: &str) -> Option<String> {
        let allowed =
            |c: char| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')' | '.');
        if input.is_empty() || !input.chars().all(allowed) {
            return None;
        }
        digits_of(input)
    }
}

fn digits_of(input: &str) -> Option<String> {
    let digits: String = input.chars().filter(char::is_ascii_digit).collect();
    (6..=15).contains(&digits.len()).then_some(digits)
}

fn query_pairs(uri: &Uri) -> Vec<(String, String)> {
    let Some(query) = uri.query() else {
        return Vec::new();
    };
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let decode = |part: &str| {
                Uri::unescape_string(&part.replace('+', " "), None)
                    .map(|decoded| decoded.to_string())
            };
            Some((decode(key)?, decode(value)?))
        })
        .collect()
}

fn lookup(pairs: &[(String, String)], key: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}

#[cfg(test)]
mod tests {
    use super::ChatLink;

    fn address_of(input: &str) -> Option<String> {
        ChatLink::parse(input).map(|link| link.address())
    }

    #[test]
    fn reads_plain_numbers() {
        assert_eq!(
            address_of("+39 333 123-4567"),
            Some("https://web.whatsapp.com/send?phone=393331234567".to_owned())
        );
    }

    #[test]
    fn reads_wa_me_links_with_text() {
        assert_eq!(
            address_of("https://wa.me/393331234567?text=Ciao%20Anna"),
            Some("https://web.whatsapp.com/send?phone=393331234567&text=Ciao%20Anna".to_owned())
        );
    }

    #[test]
    fn reads_whatsapp_scheme() {
        assert_eq!(
            address_of("whatsapp://send?phone=393331234567&text=a+b"),
            Some("https://web.whatsapp.com/send?phone=393331234567&text=a%20b".to_owned())
        );
    }

    #[test]
    fn reads_api_links() {
        assert_eq!(
            address_of("https://api.whatsapp.com/send/?phone=4915112345678"),
            Some("https://web.whatsapp.com/send?phone=4915112345678".to_owned())
        );
    }

    #[test]
    fn rejects_unrelated_input() {
        assert_eq!(
            address_of("https://example.org/send?phone=393331234567"),
            None
        );
        assert_eq!(address_of("hello"), None);
        assert_eq!(address_of("123"), None);
        assert_eq!(address_of("https://wa.me/"), None);
    }
}
