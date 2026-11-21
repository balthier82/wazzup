pub struct Scrubber {
    home: Option<String>,
    user: Option<String>,
}

impl Scrubber {
    pub fn for_this_session() -> Self {
        Self {
            home: std::env::var("HOME").ok().filter(|home| home.len() > 1),
            user: std::env::var("USER").ok().filter(|user| user.len() > 2),
        }
    }

    #[cfg(test)]
    fn with(home: &str, user: &str) -> Self {
        Self {
            home: Some(home.to_owned()),
            user: Some(user.to_owned()),
        }
    }

    pub fn clean(&self, text: &str) -> String {
        let mut cleaned = text.to_owned();
        if let Some(home) = &self.home {
            cleaned = cleaned.replace(home.as_str(), "~");
        }
        if let Some(user) = &self.user {
            cleaned = cleaned.replace(user.as_str(), "[user]");
        }
        hide_emails(&hide_numbers(&cleaned))
    }
}

fn hide_numbers(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, result: &mut String| {
        let digits = run.chars().filter(char::is_ascii_digit).count();
        if digits >= 8 {
            result.push_str("[number]");
        } else {
            result.push_str(run);
        }
        run.clear();
    };
    for character in text.chars() {
        let continues = character.is_ascii_digit()
            || (!run.is_empty() && matches!(character, ' ' | '-' | '.'))
            || (run.is_empty() && character == '+');
        if continues {
            run.push(character);
        } else {
            let trailing: String = run
                .chars()
                .rev()
                .take_while(|c| !c.is_ascii_digit())
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            run.truncate(run.len() - trailing.len());
            flush(&mut run, &mut result);
            result.push_str(&trailing);
            result.push(character);
        }
    }
    flush(&mut run, &mut result);
    result
}

fn hide_emails(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|word| {
            let core = word.trim_end();
            let looks_like_email = core.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
            });
            if looks_like_email {
                word.replacen(core, "[email]", 1)
            } else {
                word.to_owned()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Scrubber;

    fn scrubber() -> Scrubber {
        Scrubber::with("/home/anna", "anna")
    }

    #[test]
    fn hides_personal_paths() {
        assert_eq!(
            scrubber().clean("saved /home/anna/Downloads/photo.jpg"),
            "saved ~/Downloads/photo.jpg"
        );
    }

    #[test]
    fn hides_phone_numbers() {
        assert_eq!(
            scrubber().clean("chat with +39 333 123 4567 opened"),
            "chat with [number] opened"
        );
        assert_eq!(
            scrubber().clean("send?phone=393331234567&text=x"),
            "send?phone=[number]&text=x"
        );
    }

    #[test]
    fn keeps_short_numbers_and_versions() {
        assert_eq!(
            scrubber().clean("Wazzup 0.1.0 on GTK 4.14.2, retry in 15 s"),
            "Wazzup 0.1.0 on GTK 4.14.2, retry in 15 s"
        );
    }

    #[test]
    fn hides_email_addresses() {
        assert_eq!(
            scrubber().clean("contact someone@example.org now"),
            "contact [email] now"
        );
    }
}
