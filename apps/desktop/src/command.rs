//! The command line at the foot of the window: `seed 275`, `size large`, `step +1`, `random`, `reset`, `help`.
//! Parsing is separate from acting on a command so it can be tested without a window.

use tapiti_core::params::Size;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Seed(u32),
    Size(Size),
    /// A number of seeds forward (positive) or back (negative).
    Step(i64),
    Random,
    Reset,
    Help,
}

pub const HELP: &str = "seed N · size small|medium|large|huge · step ±N · random · reset";

pub fn parse(line: &str) -> Result<Command, String> {
    let mut words = line.split_whitespace();
    let Some(name) = words.next() else {
        return Err(String::new());
    };
    let arg = words.next();
    if words.next().is_some() {
        return Err(format!("too many words: {line}"));
    }
    match (name.to_lowercase().as_str(), arg) {
        ("seed", Some(n)) => n
            .parse::<u32>()
            .map(Command::Seed)
            .map_err(|_| format!("seed must be a number from 0 to {}: {n}", u32::MAX)),
        ("seed", None) => Err("seed needs a number".into()),
        ("size", Some(s)) => Size::ALL
            .iter()
            .find(|size| size.name() == s.to_lowercase())
            .map(|size| Command::Size(*size))
            .ok_or_else(|| format!("size is small, medium, large or huge: {s}")),
        ("size", None) => Err("size needs small, medium, large or huge".into()),
        ("step", arg) => match arg.unwrap_or("+1").trim_start_matches('+').parse::<i64>() {
            Ok(n) => Ok(Command::Step(n)),
            Err(_) => Err(format!(
                "step needs a number such as +1 or -3: {}",
                arg.unwrap_or("")
            )),
        },
        ("random", None) => Ok(Command::Random),
        ("reset", None) => Ok(Command::Reset),
        ("help" | "?", None) => Ok(Command::Help),
        (other, _) => Err(format!("unknown command: {other} (try help)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commands_parse() {
        assert_eq!(parse("seed 275"), Ok(Command::Seed(275)));
        assert_eq!(parse("  SEED   4294967295 "), Ok(Command::Seed(u32::MAX)));
        assert_eq!(parse("size large"), Ok(Command::Size(Size::Large)));
        assert_eq!(parse("step"), Ok(Command::Step(1)));
        assert_eq!(parse("step +3"), Ok(Command::Step(3)));
        assert_eq!(parse("step -2"), Ok(Command::Step(-2)));
        assert_eq!(parse("random"), Ok(Command::Random));
        assert_eq!(parse("reset"), Ok(Command::Reset));
        assert_eq!(parse("?"), Ok(Command::Help));
    }

    #[test]
    fn mistakes_say_what_is_wrong() {
        assert!(parse("seed").unwrap_err().contains("number"));
        assert!(parse("seed -1").unwrap_err().contains("0 to"));
        assert!(parse("seed 4294967296").is_err());
        assert!(parse("size tiny").unwrap_err().contains("small"));
        assert!(parse("step x").unwrap_err().contains("number"));
        assert!(parse("seed 1 2").unwrap_err().contains("too many"));
        assert!(parse("jump").unwrap_err().contains("unknown"));
        assert_eq!(parse("   "), Err(String::new()));
    }
}
