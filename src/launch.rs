#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchRequest {
    pub in_background: bool,
    pub quit: bool,
    pub preferences: bool,
    pub chats: Vec<String>,
    pub problems: Vec<String>,
}

impl LaunchRequest {
    pub fn parse<I, S>(arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut request = Self::default();
        let mut arguments = arguments
            .into_iter()
            .map(|argument| argument.as_ref().to_owned())
            .peekable();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--background" | "--minimized" => request.in_background = true,
                "--quit" => request.quit = true,
                "--preferences" => request.preferences = true,
                "--new-chat" => match arguments.next_if(|next| !next.starts_with("--")) {
                    Some(target) => request.chats.push(target),
                    None => request
                        .problems
                        .push("--new-chat needs a phone number or link".to_owned()),
                },
                other if other.starts_with("--new-chat=") => {
                    request.chats.push(other["--new-chat=".len()..].to_owned());
                }
                other if other.starts_with("--") => {
                    if !is_engine_switch(other) {
                        request.problems.push(format!("unknown option {other}"));
                    }
                }
                other => request.chats.push(other.to_owned()),
            }
        }
        request
    }
}

fn is_engine_switch(argument: &str) -> bool {
    const ENGINE_PREFIXES: &[&str] = &[
        "--enable-",
        "--disable-",
        "--ozone-",
        "--remote-",
        "--log-",
        "--v=",
    ];
    ENGINE_PREFIXES
        .iter()
        .any(|prefix| argument.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::LaunchRequest;

    #[test]
    fn plain_start_shows_the_window() {
        let request = LaunchRequest::parse(Vec::<String>::new());
        assert!(!request.in_background && !request.quit);
        assert_eq!(request.chats, Vec::<String>::new());
    }

    #[test]
    fn collects_chats_from_options_and_links() {
        let request = LaunchRequest::parse([
            "--new-chat",
            "+39 333 1234567",
            "whatsapp://send?phone=1234567",
            "--new-chat=https://wa.me/7654321",
        ]);
        assert_eq!(
            request.chats,
            vec![
                "+39 333 1234567",
                "whatsapp://send?phone=1234567",
                "https://wa.me/7654321"
            ]
        );
    }

    #[test]
    fn understands_background_and_quit() {
        assert!(LaunchRequest::parse(["--background"]).in_background);
        assert!(LaunchRequest::parse(["--minimized"]).in_background);
        assert!(LaunchRequest::parse(["--quit"]).quit);
        assert!(LaunchRequest::parse(["--preferences"]).preferences);
    }

    #[test]
    fn reports_problems_but_tolerates_engine_switches() {
        let request = LaunchRequest::parse(["--bogus", "--new-chat", "--enable-logging"]);
        assert_eq!(request.problems.len(), 2);
        assert_eq!(request.chats, Vec::<String>::new());
        assert_eq!(
            LaunchRequest::parse(["--disable-gpu"]).problems,
            Vec::<String>::new()
        );
    }
}
