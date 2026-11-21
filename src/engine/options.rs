use std::path::Path;

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProxySwitch {
    #[default]
    System,
    Direct,
    Server(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineOptions {
    pub proxy: ProxySwitch,
    pub spell_check: bool,
    pub spell_languages: Vec<String>,
    pub software_rendering: bool,
}

pub type Switch = (String, Option<String>);

impl EngineOptions {
    pub fn switches(&self) -> Vec<Switch> {
        let mut switches = Vec::new();
        match &self.proxy {
            ProxySwitch::System => {}
            ProxySwitch::Direct => switches.push(("no-proxy-server".to_owned(), None)),
            ProxySwitch::Server(server) => {
                switches.push(("proxy-server".to_owned(), Some(server.clone())));
            }
        }
        if self.software_rendering {
            switches.push(("disable-gpu".to_owned(), None));
            switches.push(("disable-gpu-compositing".to_owned(), None));
        }
        switches
    }

    pub fn write_profile_preferences(&self, profile_root: &Path) -> std::io::Result<()> {
        let file = profile_root.join("Default").join("Preferences");
        let Ok(source) = std::fs::read_to_string(&file) else {
            return Ok(());
        };
        let current =
            serde_json::from_str::<Value>(&source).unwrap_or_else(|_| Value::Object(Map::new()));
        std::fs::write(&file, self.merge_into(current).to_string())
    }

    fn merge_into(&self, mut preferences: Value) -> Value {
        if !preferences.is_object() {
            preferences = Value::Object(Map::new());
        }
        let languages = Value::from(self.spell_languages.clone());
        section(&mut preferences, "spellcheck").insert("dictionaries".to_owned(), languages);
        section(&mut preferences, "browser").insert(
            "enable_spellchecking".to_owned(),
            Value::Bool(self.spell_check),
        );
        preferences
    }
}

fn section<'a>(root: &'a mut Value, name: &str) -> &'a mut Map<String, Value> {
    let object = root
        .as_object_mut()
        .expect("preferences root is always an object");
    let entry = object
        .entry(name.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    entry
        .as_object_mut()
        .expect("section was just made an object")
}

#[cfg(test)]
mod tests {
    use super::{EngineOptions, ProxySwitch};
    use serde_json::json;

    fn options() -> EngineOptions {
        EngineOptions {
            proxy: ProxySwitch::System,
            spell_check: true,
            spell_languages: vec!["it-IT".to_owned(), "en-US".to_owned()],
            software_rendering: false,
        }
    }

    #[test]
    fn builds_proxy_switches() {
        assert_eq!(options().switches(), Vec::new());
        let direct = EngineOptions {
            proxy: ProxySwitch::Direct,
            ..options()
        };
        assert_eq!(
            direct.switches(),
            vec![("no-proxy-server".to_owned(), None)]
        );
        let server = EngineOptions {
            proxy: ProxySwitch::Server("socks5://h:1".to_owned()),
            ..options()
        };
        assert_eq!(
            server.switches(),
            vec![("proxy-server".to_owned(), Some("socks5://h:1".to_owned()))]
        );
    }

    #[test]
    fn disables_the_gpu_on_request() {
        let software = EngineOptions {
            software_rendering: true,
            ..options()
        };
        assert_eq!(software.switches().len(), 2);
    }

    #[test]
    fn keeps_unrelated_preferences() {
        let merged = options().merge_into(json!({
            "profile": {"name": "x"},
            "spellcheck": {"dictionaries": ["fr"], "dictionary": ""}
        }));
        assert_eq!(merged["profile"]["name"], "x");
        assert_eq!(
            merged["spellcheck"]["dictionaries"],
            json!(["it-IT", "en-US"])
        );
        assert_eq!(merged["spellcheck"]["dictionary"], "");
        assert_eq!(merged["browser"]["enable_spellchecking"], true);
    }

    #[test]
    fn repairs_unexpected_shapes() {
        let merged = options().merge_into(json!({"spellcheck": 4}));
        assert_eq!(
            merged["spellcheck"]["dictionaries"],
            json!(["it-IT", "en-US"])
        );
        assert_eq!(
            options().merge_into(json!([1]))["browser"]["enable_spellchecking"],
            true
        );
    }
}
