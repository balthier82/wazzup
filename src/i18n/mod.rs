mod catalog;

use std::fmt::Display;
use std::sync::OnceLock;

use gtk::glib;
use serde::{Deserialize, Serialize};

use catalog::{Catalog, PluralRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    #[default]
    System,
    English,
    Italian,
    French,
    German,
    Spanish,
}

impl Language {
    pub const CHOICES: [(Self, &'static str); 6] = [
        (Self::System, "System default"),
        (Self::English, "English"),
        (Self::Italian, "Italiano"),
        (Self::French, "Français"),
        (Self::German, "Deutsch"),
        (Self::Spanish, "Español"),
    ];

    fn code(self) -> Option<&'static str> {
        match self {
            Self::System | Self::English => None,
            Self::Italian => Some("it"),
            Self::French => Some("fr"),
            Self::German => Some("de"),
            Self::Spanish => Some("es"),
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "it" => Some(Self::Italian),
            "fr" => Some(Self::French),
            "de" => Some(Self::German),
            "es" => Some(Self::Spanish),
            _ => None,
        }
    }

    fn resolve(self, locales: &[String]) -> Self {
        if self != Self::System {
            return self;
        }
        locales
            .iter()
            .filter_map(|locale| locale.split(['_', '.', '@', '-']).next())
            .find_map(Self::from_code)
            .unwrap_or(Self::English)
    }

    fn source(self) -> Option<&'static str> {
        match self.code()? {
            "it" => Some(include_str!("../../po/it.po")),
            "fr" => Some(include_str!("../../po/fr.po")),
            "de" => Some(include_str!("../../po/de.po")),
            "es" => Some(include_str!("../../po/es.po")),
            _ => None,
        }
    }

    fn plural_rule(self) -> PluralRule {
        if self == Self::French {
            PluralRule::ZeroAndOneAreSingular
        } else {
            PluralRule::OneIsSingular
        }
    }
}

struct Translator {
    catalog: Catalog,
    rule: PluralRule,
}

static ACTIVE: OnceLock<Translator> = OnceLock::new();

pub fn activate(choice: Language) {
    let locales: Vec<String> = glib::language_names()
        .iter()
        .map(ToString::to_string)
        .collect();
    let language = choice.resolve(&locales);
    let catalog = language.source().map_or_default(Catalog::parse);
    tracing::debug!(
        ?language,
        entries = catalog.len(),
        "interface language selected"
    );
    let _ = ACTIVE.set(Translator {
        catalog,
        rule: language.plural_rule(),
    });
}

pub fn t(message: &str) -> String {
    ACTIVE
        .get()
        .and_then(|translator| translator.catalog.lookup(message, 0))
        .unwrap_or(message)
        .to_owned()
}

pub fn t_with(message: &str, values: &[(&str, &dyn Display)]) -> String {
    fill(&t(message), values)
}

pub fn t_count(singular: &str, plural: &str, count: u64) -> String {
    let english = if count == 1 { singular } else { plural };
    let translated = ACTIVE
        .get()
        .and_then(|translator| {
            translator
                .catalog
                .lookup(singular, translator.rule.form(count))
        })
        .unwrap_or(english);
    fill(translated, &[("count", &count)])
}

fn fill(template: &str, values: &[(&str, &dyn Display)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), &value.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::{Catalog, Language, fill};

    fn locales(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn follows_the_system_language() {
        assert_eq!(
            Language::System.resolve(&locales(&["it_IT.UTF-8", "it", "C"])),
            Language::Italian
        );
        assert_eq!(
            Language::System.resolve(&locales(&["pt_BR", "C"])),
            Language::English
        );
        assert_eq!(
            Language::German.resolve(&locales(&["it_IT"])),
            Language::German
        );
    }

    #[test]
    fn fills_named_placeholders() {
        assert_eq!(
            fill(
                "{done} of {total}",
                &[("done", &"1 MB"), ("total", &"4 MB")]
            ),
            "1 MB of 4 MB"
        );
    }

    fn placeholders(text: &str) -> Vec<&str> {
        let mut found: Vec<&str> = text
            .split('{')
            .skip(1)
            .filter_map(|piece| piece.split_once('}').map(|(name, _)| name))
            .collect();
        found.sort_unstable();
        found
    }

    #[test]
    fn every_catalog_parses_and_keeps_placeholders() {
        for (language, _) in Language::CHOICES {
            let Some(source) = language.source() else {
                continue;
            };
            let catalog = Catalog::parse(source);
            assert!(catalog.len() > 100, "{language:?} catalog looks empty");
            for (original, forms) in catalog.entries() {
                for form in forms {
                    assert_eq!(
                        placeholders(original),
                        placeholders(form),
                        "{language:?}: {original}"
                    );
                }
            }
        }
    }
}
