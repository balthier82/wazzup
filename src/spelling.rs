use gtk::glib;

pub const DICTIONARIES: [(&str, &str); 14] = [
    ("it-IT", "Italiano"),
    ("en-US", "English (US)"),
    ("en-GB", "English (UK)"),
    ("es-ES", "Español"),
    ("fr-FR", "Français"),
    ("de-DE", "Deutsch"),
    ("pt-BR", "Português (Brasil)"),
    ("pt-PT", "Português (Portugal)"),
    ("nl-NL", "Nederlands"),
    ("pl-PL", "Polski"),
    ("ru-RU", "Русский"),
    ("uk-UA", "Українська"),
    ("ro-RO", "Română"),
    ("sv-SE", "Svenska"),
];

pub fn default_languages() -> Vec<String> {
    let names: Vec<String> = glib::language_names()
        .iter()
        .map(ToString::to_string)
        .collect();
    languages_for(&names)
}

fn languages_for(locales: &[String]) -> Vec<String> {
    let mut chosen: Vec<String> = locales
        .iter()
        .filter_map(|locale| dictionary_for(locale))
        .take(1)
        .collect();
    if !chosen.iter().any(|language| language.starts_with("en-")) {
        chosen.push("en-US".to_owned());
    }
    chosen
}

fn dictionary_for(locale: &str) -> Option<String> {
    let base = locale.split(['.', '@']).next().unwrap_or_default();
    let tag = base.replace('_', "-");
    let language = tag.split('-').next().unwrap_or_default();
    DICTIONARIES
        .iter()
        .map(|(code, _)| *code)
        .find(|code| code.eq_ignore_ascii_case(&tag))
        .or_else(|| {
            DICTIONARIES
                .iter()
                .map(|(code, _)| *code)
                .find(|code| code.split('-').next() == Some(language))
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{dictionary_for, languages_for};

    fn locales(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn maps_locales_to_dictionaries() {
        assert_eq!(dictionary_for("it_IT.UTF-8"), Some("it-IT".to_owned()));
        assert_eq!(dictionary_for("pt_BR"), Some("pt-BR".to_owned()));
        assert_eq!(dictionary_for("de_AT"), Some("de-DE".to_owned()));
        assert_eq!(dictionary_for("C"), None);
    }

    #[test]
    fn always_includes_english() {
        assert_eq!(
            languages_for(&locales(&["it_IT.UTF-8", "it", "C"])),
            vec!["it-IT", "en-US"]
        );
        assert_eq!(languages_for(&locales(&["en_GB.UTF-8"])), vec!["en-GB"]);
        assert_eq!(languages_for(&locales(&["C"])), vec!["en-US"]);
    }
}
