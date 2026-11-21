pub const APP_ID: &str = "io.github.balthier82.Wazzup";
pub const APP_NAME: &str = "Wazzup";
pub const HOME_URL: &str = "https://web.whatsapp.com/";
pub const HOME_HOST: &str = "web.whatsapp.com";

const DEFAULT_PROFILE: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile(String);

impl Profile {
    pub fn from_environment() -> Self {
        Self::named(std::env::var("WAZZUP_PROFILE").ok().as_deref())
    }

    pub fn named(name: Option<&str>) -> Self {
        let cleaned: String = name
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if cleaned.is_empty() {
            Self(DEFAULT_PROFILE.to_owned())
        } else {
            Self(cleaned)
        }
    }

    pub fn is_default(&self) -> bool {
        self.0 == DEFAULT_PROFILE
    }

    pub fn application_id(&self) -> String {
        if self.is_default() {
            APP_ID.to_owned()
        } else {
            format!("{APP_ID}.{}", self.0.replace('-', "_"))
        }
    }

    pub fn engine_folder(&self) -> String {
        if self.is_default() {
            "engine".to_owned()
        } else {
            format!("engine-{}", self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Profile;

    #[test]
    fn falls_back_to_default() {
        assert!(Profile::named(None).is_default());
        assert!(Profile::named(Some("../")).is_default());
    }

    #[test]
    fn keeps_safe_characters_only() {
        assert_eq!(
            Profile::named(Some("work/../x")).engine_folder(),
            "engine-workx"
        );
    }

    #[test]
    fn derives_identifiers() {
        let work = Profile::named(Some("work-2"));
        assert_eq!(work.application_id(), "io.github.balthier82.Wazzup.work_2");
        assert_eq!(work.engine_folder(), "engine-work-2");
        assert_eq!(Profile::named(None).engine_folder(), "engine");
    }
}
