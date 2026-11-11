//! Command-bar parser: `/hunt <target>`, `/trade status`, `/brain query <text>`.

/// A parsed command from the command bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `/hunt <target>` — launch the bug-bounty swarm.
    Hunt(String),
    /// `/trade <arg>` — trade portfolio control.
    Trade(String),
    /// `/brain <arg>` — memory query / replay.
    Brain(String),
    /// `/overnight` — run the Overnight Agent now.
    Overnight,
    /// `/help`
    Help,
    /// `/quit` or `/exit`
    Quit,
    /// Empty input.
    Empty,
    /// Unrecognized command.
    Unknown(String),
}

/// Parse a command-bar line.
pub fn parse(input: &str) -> Command {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Command::Empty;
    }
    let (head, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((h, r)) => (h, r.trim()),
        None => (trimmed, ""),
    };
    match head.to_ascii_lowercase().as_str() {
        "/hunt" | "/scan" => Command::Hunt(rest.to_string()),
        "/trade" => Command::Trade(rest.to_string()),
        "/brain" | "/memory" => Command::Brain(rest.to_string()),
        "/overnight" | "/run" => Command::Overnight,
        "/help" | "/?" => Command::Help,
        "/quit" | "/exit" | "/q" => Command::Quit,
        _ => Command::Unknown(trimmed.to_string()),
    }
}

/// One-line help for the command bar.
pub fn help_line() -> &'static str {
    "/hunt <target>  /trade status  /brain query <text>  /overnight  /quit"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_documented_commands() {
        assert_eq!(parse("/hunt example.com"), Command::Hunt("example.com".into()));
        assert_eq!(parse("/trade status"), Command::Trade("status".into()));
        assert_eq!(
            parse("/brain query GSF id"),
            Command::Brain("query GSF id".into())
        );
        assert_eq!(parse("/overnight"), Command::Overnight);
        assert_eq!(parse("/quit"), Command::Quit);
        assert_eq!(parse("   "), Command::Empty);
        assert_eq!(parse("/bogus x"), Command::Unknown("/bogus x".into()));
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(parse("/HUNT a.com"), Command::Hunt("a.com".into()));
    }
}
