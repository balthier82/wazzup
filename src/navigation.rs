use gtk::glib::Uri;
use gtk::glib::UriFlags;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Inside,
    SystemBrowser,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct NavigationPolicy {
    trusted_host: String,
    passthrough_schemes: Vec<&'static str>,
}

impl NavigationPolicy {
    pub fn new(trusted_host: impl Into<String>) -> Self {
        Self {
            trusted_host: trusted_host.into(),
            passthrough_schemes: vec!["about", "blob", "data"],
        }
    }

    pub fn classify(&self, address: &str) -> Destination {
        let Ok(uri) = Uri::parse(address, UriFlags::NONE) else {
            return Destination::Blocked;
        };
        let scheme = uri.scheme().to_ascii_lowercase();
        if self.passthrough_schemes.contains(&scheme.as_str()) {
            return Destination::Inside;
        }
        if scheme != "https" && scheme != "http" {
            return Destination::Blocked;
        }
        let is_trusted = uri
            .host()
            .is_some_and(|host| host.eq_ignore_ascii_case(&self.trusted_host));
        match (is_trusted, scheme.as_str()) {
            (true, "https") => Destination::Inside,
            (true, _) => Destination::Blocked,
            (false, _) => Destination::SystemBrowser,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Destination, NavigationPolicy};

    fn policy() -> NavigationPolicy {
        NavigationPolicy::new("web.whatsapp.com")
    }

    #[test]
    fn keeps_trusted_host_inside() {
        assert_eq!(
            policy().classify("https://web.whatsapp.com/"),
            Destination::Inside
        );
        assert_eq!(
            policy().classify("https://WEB.WhatsApp.com/send"),
            Destination::Inside
        );
    }

    #[test]
    fn sends_foreign_web_pages_outside() {
        assert_eq!(
            policy().classify("https://example.org/x"),
            Destination::SystemBrowser
        );
        assert_eq!(
            policy().classify("https://web.whatsapp.com.evil.test/"),
            Destination::SystemBrowser
        );
    }

    #[test]
    fn blocks_plain_http_on_trusted_host() {
        assert_eq!(
            policy().classify("http://web.whatsapp.com/"),
            Destination::Blocked
        );
    }

    #[test]
    fn blocks_unknown_schemes() {
        assert_eq!(
            policy().classify("file:///etc/passwd"),
            Destination::Blocked
        );
        assert_eq!(policy().classify("not a uri"), Destination::Blocked);
    }

    #[test]
    fn allows_internal_schemes() {
        assert_eq!(
            policy().classify("blob:https://web.whatsapp.com/abc"),
            Destination::Inside
        );
        assert_eq!(policy().classify("about:blank"), Destination::Inside);
    }
}
