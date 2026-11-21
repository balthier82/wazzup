use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralRule {
    OneIsSingular,
    ZeroAndOneAreSingular,
}

impl PluralRule {
    pub fn form(self, count: u64) -> usize {
        let plural = match self {
            Self::OneIsSingular => count != 1,
            Self::ZeroAndOneAreSingular => count > 1,
        };
        usize::from(plural)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    entries: HashMap<String, Vec<String>>,
}

#[derive(Default)]
struct Pending {
    id: Option<String>,
    forms: Vec<(usize, String)>,
    target: Target,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Target {
    #[default]
    Nothing,
    Id,
    PluralId,
    Form(usize),
}

impl Catalog {
    pub fn parse(source: &str) -> Self {
        let mut catalog = Self::default();
        let mut pending = Pending::default();
        for line in source.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("msgid_plural ") {
                pending.target = Target::PluralId;
                let _ = unquote(rest);
                continue;
            }
            if let Some(rest) = line.strip_prefix("msgid ") {
                catalog.finish(&mut pending);
                pending.id = unquote(rest);
                pending.target = Target::Id;
                continue;
            }
            if let Some(rest) = line.strip_prefix("msgstr[") {
                let Some((index, text)) = rest.split_once("] ") else {
                    continue;
                };
                let index = index.parse().unwrap_or(0);
                pending
                    .forms
                    .push((index, unquote(text).unwrap_or_default()));
                pending.target = Target::Form(index);
                continue;
            }
            if let Some(rest) = line.strip_prefix("msgstr ") {
                pending.forms.push((0, unquote(rest).unwrap_or_default()));
                pending.target = Target::Form(0);
                continue;
            }
            if line.starts_with('"') {
                let piece = unquote(line).unwrap_or_default();
                match pending.target {
                    Target::Id => {
                        if let Some(id) = pending.id.as_mut() {
                            id.push_str(&piece);
                        }
                    }
                    Target::Form(index) => {
                        if let Some((_, text)) = pending
                            .forms
                            .iter_mut()
                            .rev()
                            .find(|(form, _)| *form == index)
                        {
                            text.push_str(&piece);
                        }
                    }
                    Target::PluralId | Target::Nothing => {}
                }
            }
        }
        catalog.finish(&mut pending);
        catalog
    }

    fn finish(&mut self, pending: &mut Pending) {
        let taken = std::mem::take(pending);
        let Some(id) = taken.id.filter(|id| !id.is_empty()) else {
            return;
        };
        let mut forms = taken.forms;
        forms.sort_by_key(|(index, _)| *index);
        let forms: Vec<String> = forms.into_iter().map(|(_, text)| text).collect();
        if forms.iter().any(|text| !text.is_empty()) {
            self.entries.insert(id, forms);
        }
    }

    pub fn lookup(&self, id: &str, form: usize) -> Option<&str> {
        let forms = self.entries.get(id)?;
        forms
            .get(form)
            .or_else(|| forms.first())
            .map(String::as_str)
            .filter(|text| !text.is_empty())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn entries(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.entries.iter()
    }
}

fn unquote(raw: &str) -> Option<String> {
    let inner = raw.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut text = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            text.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => text.push('\n'),
            Some('t') => text.push('\t'),
            Some(other) => text.push(other),
            None => {}
        }
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::{Catalog, PluralRule};

    const SAMPLE: &str = r#"
# comment
msgid ""
msgstr "Content-Type: text/plain; charset=UTF-8\n"

msgid "Quit"
msgstr "Esci"

msgid "Long "
"line"
msgstr "Riga "
"lunga"

msgid "Say \"hi\""
msgstr "Di' \"ciao\""

msgid "{count} unread chat"
msgid_plural "{count} unread chats"
msgstr[0] "{count} chat non letta"
msgstr[1] "{count} chat non lette"

msgid "Untranslated"
msgstr ""
"#;

    #[test]
    fn reads_simple_and_multiline_entries() {
        let catalog = Catalog::parse(SAMPLE);
        assert_eq!(catalog.lookup("Quit", 0), Some("Esci"));
        assert_eq!(catalog.lookup("Long line", 0), Some("Riga lunga"));
        assert_eq!(catalog.lookup("Say \"hi\"", 0), Some("Di' \"ciao\""));
    }

    #[test]
    fn reads_plural_forms() {
        let catalog = Catalog::parse(SAMPLE);
        assert_eq!(
            catalog.lookup("{count} unread chat", 1),
            Some("{count} chat non lette")
        );
    }

    #[test]
    fn skips_header_and_empty_translations() {
        let catalog = Catalog::parse(SAMPLE);
        assert_eq!(catalog.lookup("", 0), None);
        assert_eq!(catalog.lookup("Untranslated", 0), None);
        assert_eq!(catalog.len(), 4);
    }

    #[test]
    fn chooses_plural_forms_per_language() {
        assert_eq!(PluralRule::OneIsSingular.form(0), 1);
        assert_eq!(PluralRule::OneIsSingular.form(1), 0);
        assert_eq!(PluralRule::ZeroAndOneAreSingular.form(0), 0);
        assert_eq!(PluralRule::ZeroAndOneAreSingular.form(2), 1);
    }
}
